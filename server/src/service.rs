use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;
use crate::{connectors, directory, error::ApiError, geocoding, matching, models::{ImportResult, Job, SourceSummary, Posting, Profile, ProfileRow, ResolvedLocation, Source, StoredPosting}, notifications, AppState};

pub async fn profiles(state:&AppState,user:Uuid)->Result<Vec<Profile>,ApiError>{
 let rows=sqlx::query_as::<_,ProfileRow>("SELECT id,name,titles,required_skills,preferred_skills,remote_only,countries,regions,cities,min_score,enabled,schedule,resume_text,weights,distance_enabled,active_location,active_latitude,active_longitude,maximum_distance_km,include_remote,include_hybrid,include_onsite,broad_location_policy FROM profiles WHERE user_id=$1 ORDER BY updated_at DESC").bind(user).fetch_all(&state.db).await?;
 rows.into_iter().map(|r|r.try_into().map_err(ApiError::from)).collect()
}

pub async fn sources(state:&AppState,user:Uuid)->Result<Vec<Source>,ApiError>{Ok(sqlx::query_as("SELECT s.id,s.kind,s.company,s.board_token,s.location,(s.enabled AND COALESCE(d.enabled,true)) enabled,s.last_refreshed_at,s.last_error,s.request_delay_ms,s.max_retries,s.max_results,s.page_size,s.max_calls,s.hydration_limit FROM sources s LEFT JOIN company_directory d ON lower(btrim(d.company_name))=lower(btrim(s.company)) WHERE s.user_id=$1 ORDER BY s.company").bind(user).fetch_all(&state.db).await?)}

pub async fn jobs(state:&AppState,user:Uuid)->Result<Vec<Job>,ApiError>{let stale:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jobs j WHERE j.user_id=$1 AND j.active=true AND j.scoring_version<>$2 AND NOT EXISTS(SELECT 1 FROM company_directory d WHERE d.enabled=false AND (lower(btrim(d.company_name))=lower(btrim(j.company)) OR EXISTS(SELECT 1 FROM sources s WHERE s.user_id=j.user_id AND s.id=j.source_id AND lower(btrim(d.company_name))=lower(btrim(s.company))))))").bind(user).bind(matching::SCORING_VERSION).fetch_one(&state.db).await?;if stale{rescore_saved_jobs(state,user).await?;}Ok(sqlx::query_as("SELECT j.id,j.source,j.external_id,j.title,j.company,j.location,j.remote_type,j.posted_at,j.apply_url,j.description,j.skills,j.match_score,j.match_reason,j.profile_matches,j.is_new,COALESCE(t.status,j.status) status,j.distance_km,j.location_status FROM jobs j LEFT JOIN trackers t ON t.user_id=j.user_id AND t.job_id=j.id WHERE j.user_id=$1 AND j.active=true AND NOT EXISTS(SELECT 1 FROM company_directory d WHERE d.enabled=false AND (lower(btrim(d.company_name))=lower(btrim(j.company)) OR EXISTS(SELECT 1 FROM sources s WHERE s.user_id=j.user_id AND s.id=j.source_id AND lower(btrim(d.company_name))=lower(btrim(s.company))))) ORDER BY j.match_score DESC,j.posted_at DESC").bind(user).fetch_all(&state.db).await?)}

pub async fn rescore_saved_jobs(state:&AppState,user:Uuid)->Result<usize,ApiError>{
 let enabled=profiles(state,user).await?;if enabled.is_empty(){let result=sqlx::query("UPDATE jobs SET active=false,scoring_version=$2 WHERE user_id=$1 AND active=true").bind(user).bind(matching::SCORING_VERSION).execute(&state.db).await?;return Ok(result.rows_affected() as usize);}
 let rows=sqlx::query_as::<_,StoredPosting>("SELECT j.id,j.external_id,j.title,j.company,j.location,j.remote_type,j.posted_at,j.apply_url,j.description,j.location_candidates FROM jobs j WHERE j.user_id=$1 AND j.active=true AND NOT EXISTS(SELECT 1 FROM company_directory d WHERE d.enabled=false AND (lower(btrim(d.company_name))=lower(btrim(j.company)) OR EXISTS(SELECT 1 FROM sources s WHERE s.user_id=j.user_id AND s.id=j.source_id AND lower(btrim(d.company_name))=lower(btrim(s.company)))))").bind(user).fetch_all(&state.db).await?;let mut updated=0;
 for row in rows{let row_id=row.id.clone();let posting=Posting{external_id:row.external_id,title:row.title,company:row.company,location:row.location,remote_type:row.remote_type,posted_at:row.posted_at,apply_url:row.apply_url,description:row.description};let mut resolved:Vec<ResolvedLocation>=serde_json::from_value(row.location_candidates).unwrap_or_default();if resolved.is_empty()&&posting.remote_type!="Remote"{resolved=geocoding::resolve_posting(state,&posting).await?;sqlx::query("UPDATE jobs SET location_candidates=$3 WHERE user_id=$1 AND id=$2").bind(user).bind(&row_id).bind(json!(&resolved)).execute(&state.db).await?;}let mut matches=Vec::new();for profile in &enabled{let evaluation=matching::evaluate_location(&posting,profile,&resolved);if !evaluation.eligible{continue;}let scored=matching::score_with_location(&posting,profile,&evaluation);if scored.match_score>=profile.min_score{matches.push(scored);}}matches.sort_by_key(|m|-m.match_score);
  if matches.is_empty(){sqlx::query("UPDATE jobs SET active=false,scoring_version=$3 WHERE user_id=$1 AND id=$2").bind(user).bind(&row_id).bind(matching::SCORING_VERSION).execute(&state.db).await?;continue;}
  let best=&matches[0];let skills=unique_strings(matches.iter().flat_map(|m|m.matched_terms.iter().cloned()).collect());sqlx::query("UPDATE jobs SET skills=$3,match_score=$4,match_reason=$5,profile_matches=$6,scoring_version=$7,distance_km=$8,location_status=$9 WHERE user_id=$1 AND id=$2").bind(user).bind(&row_id).bind(json!(skills)).bind(best.match_score).bind(&best.match_reason).bind(json!(&matches)).bind(matching::SCORING_VERSION).bind(best.distance_km).bind(&best.location_status).execute(&state.db).await?;updated+=1;
 }
 Ok(updated)
}

fn unique_strings(mut values:Vec<String>)->Vec<String>{values.sort_by_key(|v|v.to_lowercase());values.dedup_by(|a,b|a.eq_ignore_ascii_case(b));values}

/// A running search that has not reported progress for this long is treated as stopped.
const STALE_RUN_MINUTES:i32=30;
/// Progress is saved after this many processed jobs, which also acts as the run's heartbeat.
const PROGRESS_EVERY:usize=25;
const ALREADY_RUNNING:&str="A search is already running. Wait for it to finish before starting another.";

/// Profiles and sources checked before a search starts, so problems are reported immediately.
struct SearchPlan{profiles:Vec<Profile>,sources:Vec<Source>}

fn now_text()->String{Utc::now().to_rfc3339()}

async fn plan_search(state:&AppState,user:Uuid,only_source:Option<&str>)->Result<SearchPlan,ApiError>{
 let enabled_profiles=profiles(state,user).await?;
 if enabled_profiles.is_empty(){return Err(ApiError::BadRequest("Create at least one profile first.".into()));}
 let mut source_rows=sources(state,user).await?.into_iter().filter(|s|s.enabled).collect::<Vec<_>>();if let Some(id)=only_source{source_rows.retain(|s|s.id==id);}
 if source_rows.is_empty(){return Err(ApiError::BadRequest("Enable at least one source first.".into()));}
 Ok(SearchPlan{profiles:enabled_profiles,sources:source_rows})
}

/// Marks running searches that stopped reporting progress as abandoned, so they no longer block new searches.
pub async fn abandon_stale_runs(state:&AppState,user:Option<Uuid>)->Result<u64,ApiError>{
 let result=sqlx::query("UPDATE search_runs SET status='Abandoned',finished_at=$2,progress='Stopped before finishing',errors=errors||'[\"The search stopped responding before it finished. Run it again.\"]'::jsonb WHERE status='Running' AND ($1::uuid IS NULL OR user_id=$1) AND COALESCE(heartbeat_at,'-infinity'::timestamptz)<now()-make_interval(mins=>$3)")
  .bind(user).bind(now_text()).bind(STALE_RUN_MINUTES).execute(&state.db).await?;
 Ok(result.rows_affected())
}

/// Called when a process starts. Searches that process owned cannot still be running after a restart:
/// the API owns manual searches and the worker owns scheduled ones.
pub async fn abandon_interrupted_runs(state:&AppState,scheduled:bool)->Result<u64,ApiError>{
 let result=sqlx::query("UPDATE search_runs SET status='Abandoned',finished_at=$1,progress='Stopped before finishing',errors=errors||'[\"Job Tracker restarted before this search finished. Run it again.\"]'::jsonb WHERE status='Running' AND (trigger_type='scheduled')=$2")
  .bind(now_text()).bind(scheduled).execute(&state.db).await?;
 Ok(result.rows_affected())
}

/// Records a new running search. The database allows one running search per user, so a second
/// request is refused instead of starting duplicate work.
async fn begin_run(state:&AppState,user:Uuid,trigger:&str,sources_checked:usize)->Result<String,ApiError>{
 abandon_stale_runs(state,Some(user)).await?;
 let run_id=Uuid::new_v4().to_string();
 let inserted=sqlx::query("INSERT INTO search_runs(user_id,id,started_at,trigger_type,sources_checked,status,progress,heartbeat_at) VALUES($1,$2,$3,$4,$5,'Running','Starting',now()) ON CONFLICT DO NOTHING")
  .bind(user).bind(&run_id).bind(now_text()).bind(trigger).bind(sources_checked as i32).execute(&state.db).await?;
 if inserted.rows_affected()==0{return Err(ApiError::Conflict(ALREADY_RUNNING.into()));}
 Ok(run_id)
}

async fn report_progress(state:&AppState,user:Uuid,run_id:&str,progress:&str)->Result<(),ApiError>{
 sqlx::query("UPDATE search_runs SET progress=$3,heartbeat_at=now() WHERE user_id=$1 AND id=$2 AND status='Running'").bind(user).bind(run_id).bind(progress).execute(&state.db).await?;
 Ok(())
}

async fn record_failure(state:&AppState,user:Uuid,run_id:&str,message:&str){
 if let Err(error)=sqlx::query("UPDATE search_runs SET status='Failed',finished_at=$3,errors=errors||$4,progress='Failed',heartbeat_at=now() WHERE user_id=$1 AND id=$2 AND status='Running'").bind(user).bind(run_id).bind(now_text()).bind(json!([message])).execute(&state.db).await{tracing::error!(?error,%user,run=%run_id,"could not record the failed search");}
}

/// Starts a search in the background and returns its run id immediately. The search keeps going
/// if the page reloads or closes; the page follows its progress through the search-runs API.
pub async fn start_search(state:&AppState,user:Uuid,trigger:&str,only_source:Option<&str>)->Result<String,ApiError>{
 let plan=plan_search(state,user,only_source).await?;
 let run_id=begin_run(state,user,trigger,plan.sources.len()).await?;
 let task_state=state.clone();let task_run=run_id.clone();let task_trigger=trigger.to_string();
 let search=tokio::spawn(async move{execute_and_record(&task_state,user,&task_run,&task_trigger,plan).await});
 let watch_state=state.clone();let watch_run=run_id.clone();
 tokio::spawn(async move{
  match search.await{
   Ok(Ok(_))=>{},
   Ok(Err(error))=>tracing::error!(%user,run=%watch_run,%error,"background search failed"),
   // A panic would otherwise leave the run marked as running until it went stale.
   Err(error)=>{tracing::error!(%user,run=%watch_run,%error,"background search stopped unexpectedly");record_failure(&watch_state,user,&watch_run,"The search stopped unexpectedly. Run it again.").await;}
  }
 });
 Ok(run_id)
}

/// Runs a search to completion in the current task. The scheduled worker uses this.
pub async fn run_search(state:&AppState,user:Uuid,trigger:&str,only_source:Option<&str>)->Result<ImportResult,ApiError>{
 let plan=plan_search(state,user,only_source).await?;
 let run_id=begin_run(state,user,trigger,plan.sources.len()).await?;
 execute_and_record(state,user,&run_id,trigger,plan).await
}

async fn execute_and_record(state:&AppState,user:Uuid,run_id:&str,trigger:&str,plan:SearchPlan)->Result<ImportResult,ApiError>{
 match execute_search(state,user,run_id,plan).await{
  Ok(result)=>{
   sqlx::query("UPDATE search_runs SET status='Completed',finished_at=$3,jobs_imported=$4,errors=$5,result=$6,progress='Finished',heartbeat_at=now() WHERE user_id=$1 AND id=$2")
    .bind(user).bind(run_id).bind(now_text()).bind(result.imported as i32).bind(json!(&result.source_errors)).bind(json!(&result)).execute(&state.db).await?;
   if trigger=="scheduled"{notifications::create(state,user,"Job Tracker search complete",&format!("{} active jobs imported; {} jobs marked inactive.",result.imported,result.inactive),"/").await?;}
   Ok(result)
  }
  Err(error)=>{record_failure(state,user,run_id,&error.to_string()).await;Err(error)}
 }
}

async fn execute_search(state:&AppState,user:Uuid,run_id:&str,plan:SearchPlan)->Result<ImportResult,ApiError>{
 let SearchPlan{profiles:enabled_profiles,sources:source_rows}=plan;
 let mut imported=0usize;let mut inactive=0usize;let mut fetched=0usize;let mut available=0usize;let mut outside_location=0usize;let mut below_minimum_score=0usize;let mut errors=Vec::new();let mut truncated_sources=Vec::new();let mut summaries=Vec::new();
 let source_total=source_rows.len();
 for (source_index,source) in source_rows.into_iter().enumerate() {
  let source_label=format!("{} (source {} of {})",source.company,source_index+1,source_total);
  report_progress(state,user,run_id,&format!("{source_label}: fetching jobs")).await?;
  let (imported_before,fetched_before,available_before,outside_before,below_before)=(imported,fetched,available,outside_location,below_minimum_score);
  let mut source_warning:Option<String>=None;let mut source_error:Option<String>=None;
  let encrypted:Option<String>=sqlx::query_scalar("SELECT credentials_ciphertext FROM sources WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).fetch_one(&state.db).await?;
  match connectors::fetch_with_metadata(state,&source,encrypted.as_deref()).await {
   Ok(result)=>{
    source_warning=result.warning.clone();
    let postings=result.postings;let reported=result.reported_count.map(|count|count.max(0)as usize).unwrap_or(postings.len());available+=reported;if reported>postings.len(){truncated_sources.push(format!("{} ({} of {})",source.company,postings.len(),reported));}
    fetched+=postings.len();
    let seen=postings.iter().map(|p|format!("{}:{}",source.kind,p.external_id)).collect::<Vec<_>>();
    let posting_total=postings.len();
    for (posting_index,posting) in postings.into_iter().enumerate() {
     if posting_index%PROGRESS_EVERY==0{report_progress(state,user,run_id,&format!("{source_label}: checking job {} of {}",posting_index+1,posting_total)).await?;}
     let resolved=geocoding::resolve_posting(state,&posting).await?;let mut matches=Vec::new();let mut eligible_count=0usize;
     for profile in &enabled_profiles{let evaluation=matching::evaluate_location(&posting,profile,&resolved);if !evaluation.eligible{continue;}eligible_count+=1;let scored=matching::score_with_location(&posting,profile,&evaluation);if scored.match_score>=profile.min_score{matches.push(scored);}}
     if eligible_count==0{outside_location+=1;continue;}
     if matches.is_empty(){below_minimum_score+=1;continue;}
     matches.sort_by_key(|m|-m.match_score);if matches.is_empty(){continue;}let best=&matches[0];let id=format!("{}:{}",source.kind,posting.external_id);
     let skills=unique_strings(matches.iter().flat_map(|m|m.matched_terms.iter().cloned()).collect());
     sqlx::query("INSERT INTO jobs(user_id,id,source,external_id,source_id,title,company,location,remote_type,posted_at,apply_url,description,skills,match_score,match_reason,profile_matches,is_new,active,last_seen_at,location_candidates,distance_km,location_status,scoring_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,true,true,now(),$17,$18,$19,$20) ON CONFLICT(user_id,id) DO UPDATE SET title=excluded.title,company=excluded.company,location=excluded.location,remote_type=excluded.remote_type,posted_at=excluded.posted_at,apply_url=excluded.apply_url,description=excluded.description,skills=excluded.skills,match_score=excluded.match_score,match_reason=excluded.match_reason,profile_matches=excluded.profile_matches,is_new=false,active=true,last_seen_at=now(),location_candidates=excluded.location_candidates,distance_km=excluded.distance_km,location_status=excluded.location_status,scoring_version=excluded.scoring_version")
      .bind(user).bind(&id).bind(&source.kind).bind(&posting.external_id).bind(&source.id).bind(&posting.title).bind(&posting.company).bind(&posting.location).bind(&posting.remote_type).bind(&posting.posted_at).bind(&posting.apply_url).bind(&posting.description).bind(json!(skills)).bind(best.match_score).bind(&best.match_reason).bind(json!(&matches)).bind(json!(&resolved)).bind(best.distance_km).bind(&best.location_status).bind(matching::SCORING_VERSION).execute(&state.db).await?;imported+=1;
    }
    if result.warning.is_none(){let update=if seen.is_empty(){sqlx::query("UPDATE jobs SET active=false WHERE user_id=$1 AND source_id=$2 AND active=true").bind(user).bind(&source.id).execute(&state.db).await?}else{sqlx::query("UPDATE jobs SET active=false WHERE user_id=$1 AND source_id=$2 AND active=true AND NOT(id=ANY($3))").bind(user).bind(&source.id).bind(&seen).execute(&state.db).await?};inactive+=update.rows_affected() as usize;}
    if let Some(warning)=result.warning{errors.push(format!("{}: {warning}",source.company));sqlx::query("UPDATE sources SET last_refreshed_at=now(),last_error=$3 WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).bind(warning).execute(&state.db).await?;}else{sqlx::query("UPDATE sources SET last_refreshed_at=now(),last_error=NULL WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).execute(&state.db).await?;}
   },Err(error)=>{let message=error.to_string();source_error=Some(message.clone());errors.push(format!("{}: {message}",source.company));sqlx::query("UPDATE sources SET last_error=$3 WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).bind(message).execute(&state.db).await?;}
  }
  summaries.push(SourceSummary{source_id:source.id.clone(),company:source.company.clone(),kind:source.kind.clone(),available:available-available_before,fetched:fetched-fetched_before,imported:imported-imported_before,outside_location:outside_location-outside_before,below_minimum_score:below_minimum_score-below_before,warning:source_warning,error:source_error});
 }
 report_progress(state,user,run_id,"Updating match scores").await?;
 rescore_saved_jobs(state,user).await?;
 Ok(ImportResult{imported,inactive,fetched,available,outside_location,below_minimum_score,source_errors:errors,truncated_sources,sources:summaries})
}

pub async fn worker_tick(state:&AppState)->Result<(),ApiError>{
 directory::verify_due(state).await?;
 sqlx::query("DELETE FROM connector_detail_cache WHERE last_used_at<now()-interval '90 days'").execute(&state.db).await?;
 let users:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT user_id FROM profiles WHERE schedule<>'Manual' AND (next_run_at IS NULL OR next_run_at<=now())").fetch_all(&state.db).await?;
 for user in users { match run_search(state,user,"scheduled",None).await{Ok(_)=>{},Err(ApiError::Conflict(_))=>tracing::info!(%user,"scheduled search skipped because a search is already running"),Err(error)=>{tracing::error!(%user,%error,"scheduled search failed");notifications::create(state,user,"Job Tracker scheduled search failed",&error.to_string(),"/").await?;}} sqlx::query("UPDATE profiles SET next_run_at=CASE schedule WHEN 'Daily' THEN now()+interval '1 day' WHEN 'Weekdays' THEN now()+CASE extract(isodow from now()) WHEN 5 THEN interval '3 days' WHEN 6 THEN interval '2 days' ELSE interval '1 day' END WHEN 'Weekly' THEN now()+interval '7 days' ELSE NULL END WHERE user_id=$1 AND schedule<>'Manual' AND (next_run_at IS NULL OR next_run_at<=now())").bind(user).execute(&state.db).await?; }
 let due=sqlx::query_as::<_,(Uuid,i64)>("SELECT user_id,count(*) FROM trackers WHERE follow_up_at IS NOT NULL AND follow_up_at<>'' AND follow_up_at::timestamptz<=now() GROUP BY user_id").fetch_all(&state.db).await?;
 for(user,count)in due{let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE user_id=$1 AND title='Job Tracker follow-ups due' AND created_at>current_date)").bind(user).fetch_one(&state.db).await?;if !exists{notifications::create(state,user,"Job Tracker follow-ups due",&format!("{count} application follow-up(s) are due."),"/").await?;}}
 Ok(())
}

pub fn as_array(value:Value)->Vec<String>{serde_json::from_value(value).unwrap_or_default()}
