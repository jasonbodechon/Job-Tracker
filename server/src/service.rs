use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;
use crate::{connectors, directory, error::ApiError, geocoding, matching, models::{ImportResult, Job, Posting, Profile, ProfileRow, ResolvedLocation, Source, StoredPosting}, notifications, AppState};

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

pub async fn run_search(state:&AppState,user:Uuid,trigger:&str,only_source:Option<&str>)->Result<ImportResult,ApiError>{
 let enabled_profiles=profiles(state,user).await?;
 if enabled_profiles.is_empty(){return Err(ApiError::BadRequest("Create at least one profile first.".into()));}
 let mut source_rows=sources(state,user).await?.into_iter().filter(|s|s.enabled).collect::<Vec<_>>();if let Some(id)=only_source{source_rows.retain(|s|s.id==id);}
 if source_rows.is_empty(){return Err(ApiError::BadRequest("Enable at least one source first.".into()));}
 let run_id=Uuid::new_v4().to_string();let started=Utc::now().to_rfc3339();
 sqlx::query("INSERT INTO search_runs(user_id,id,started_at,trigger_type,sources_checked) VALUES($1,$2,$3,$4,$5)").bind(user).bind(&run_id).bind(&started).bind(trigger).bind(source_rows.len() as i32).execute(&state.db).await?;
 let mut imported=0usize;let mut inactive=0usize;let mut fetched=0usize;let mut available=0usize;let mut outside_location=0usize;let mut below_minimum_score=0usize;let mut errors=Vec::new();let mut truncated_sources=Vec::new();
 for source in source_rows {
  let encrypted:Option<String>=sqlx::query_scalar("SELECT credentials_ciphertext FROM sources WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).fetch_one(&state.db).await?;
  match connectors::fetch_with_metadata(state,&source,encrypted.as_deref()).await {
   Ok(result)=>{
    let postings=result.postings;let reported=result.reported_count.map(|count|count.max(0)as usize).unwrap_or(postings.len());available+=reported;if reported>postings.len(){truncated_sources.push(format!("{} ({} of {})",source.company,postings.len(),reported));}
    fetched+=postings.len();
    let seen=postings.iter().map(|p|format!("{}:{}",source.kind,p.external_id)).collect::<Vec<_>>();
    for posting in postings {
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
   },Err(error)=>{let message=error.to_string();errors.push(format!("{}: {message}",source.company));sqlx::query("UPDATE sources SET last_error=$3 WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).bind(message).execute(&state.db).await?;}
  }
 }
 sqlx::query("UPDATE search_runs SET finished_at=$3,jobs_imported=$4,errors=$5 WHERE user_id=$1 AND id=$2").bind(user).bind(&run_id).bind(Utc::now().to_rfc3339()).bind(imported as i32).bind(json!(&errors)).execute(&state.db).await?;
 rescore_saved_jobs(state,user).await?;
 if trigger=="scheduled"{notifications::create(state,user,"Job Tracker search complete",&format!("{imported} active jobs imported; {inactive} jobs marked inactive."),"/").await?;}
 Ok(ImportResult{imported,inactive,fetched,available,outside_location,below_minimum_score,source_errors:errors,truncated_sources})
}

pub async fn worker_tick(state:&AppState)->Result<(),ApiError>{
 directory::verify_due(state).await?;
 sqlx::query("DELETE FROM connector_detail_cache WHERE last_used_at<now()-interval '90 days'").execute(&state.db).await?;
 let users:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT user_id FROM profiles WHERE schedule<>'Manual' AND (next_run_at IS NULL OR next_run_at<=now())").fetch_all(&state.db).await?;
 for user in users { if let Err(error)=run_search(state,user,"scheduled",None).await{tracing::error!(%user,%error,"scheduled search failed");notifications::create(state,user,"Job Tracker scheduled search failed",&error.to_string(),"/").await?;} sqlx::query("UPDATE profiles SET next_run_at=CASE schedule WHEN 'Daily' THEN now()+interval '1 day' WHEN 'Weekdays' THEN now()+CASE extract(isodow from now()) WHEN 5 THEN interval '3 days' WHEN 6 THEN interval '2 days' ELSE interval '1 day' END WHEN 'Weekly' THEN now()+interval '7 days' ELSE NULL END WHERE user_id=$1 AND schedule<>'Manual' AND (next_run_at IS NULL OR next_run_at<=now())").bind(user).execute(&state.db).await?; }
 let due=sqlx::query_as::<_,(Uuid,i64)>("SELECT user_id,count(*) FROM trackers WHERE follow_up_at IS NOT NULL AND follow_up_at<>'' AND follow_up_at::timestamptz<=now() GROUP BY user_id").fetch_all(&state.db).await?;
 for(user,count)in due{let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notifications WHERE user_id=$1 AND title='Job Tracker follow-ups due' AND created_at>current_date)").bind(user).fetch_one(&state.db).await?;if !exists{notifications::create(state,user,"Job Tracker follow-ups due",&format!("{count} application follow-up(s) are due."),"/").await?;}}
 Ok(())
}

pub fn as_array(value:Value)->Vec<String>{serde_json::from_value(value).unwrap_or_default()}
