use crate::{connectors, error::ApiError, models::{CompanyDirectoryEntry, Source}, AppState};

pub async fn entries(state:&AppState,_user:uuid::Uuid)->Result<Vec<CompanyDirectoryEntry>,ApiError>{
 Ok(sqlx::query_as("SELECT id,enabled,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,current_job_count,last_verified_at,verification_status,verification_error,identifier_origin,detected_provider,redirect_url FROM company_directory ORDER BY company_name").fetch_all(&state.db).await?)
}

pub async fn add_source(state:&AppState,user:uuid::Uuid,id:&str)->Result<Source,ApiError>{
 let entry:CompanyDirectoryEntry=sqlx::query_as("SELECT id,true enabled,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,current_job_count,last_verified_at,verification_status,verification_error,identifier_origin,detected_provider,redirect_url FROM company_directory WHERE id=$1").bind(id).fetch_optional(&state.db).await?.ok_or_else(||ApiError::BadRequest("Company directory entry was not found.".into()))?;
 let company_enabled:bool=sqlx::query_scalar("SELECT enabled FROM company_directory WHERE id=$1").bind(id).fetch_one(&state.db).await?;if !company_enabled{return Err(ApiError::BadRequest("Enable this company before adding its source.".into()));}
 // A manually verified identifier is never silently replaced by automatic detection.
 let kind=if entry.identifier_origin=="Automatic"{entry.detected_provider.clone().unwrap_or(entry.ats_provider.clone())}else{entry.ats_provider.clone()};
 let board_token=match kind.as_str(){"Workday"|"CareerPage"=>entry.careers_url.clone(),_=>entry.board_identifier.clone()};
 let source_location=if kind=="Adzuna"{entry.countries.as_array().and_then(|values|values.first()).and_then(|value|value.as_str()).map(str::to_string)}else{None};
 let existing:Vec<Source>=sqlx::query_as("SELECT id,kind,company,board_token,location,enabled,last_refreshed_at,last_error,request_delay_ms,max_retries,max_results,page_size,max_calls,hydration_limit FROM sources WHERE user_id=$1 ORDER BY enabled DESC, id").bind(user).fetch_all(&state.db).await?;
 let mut equivalents=existing.into_iter().filter(|source|same_identity(&source.company,&entry.company_name)&&source.kind==kind&&same_identity(&source.board_token,&board_token));
 if let Some(mut source)=equivalents.next(){
  for duplicate in equivalents{sqlx::query("DELETE FROM sources WHERE user_id=$1 AND id=$2").bind(user).bind(duplicate.id).execute(&state.db).await?;}
  if !source.enabled{sqlx::query("UPDATE sources SET enabled=true,updated_at=now() WHERE user_id=$1 AND id=$2").bind(user).bind(&source.id).execute(&state.db).await?;source.enabled=true;}
  return Ok(source);
 }
 let source=Source{id:format!("directory-{}",entry.id),kind,company:entry.company_name,board_token,location:source_location,enabled:true,last_refreshed_at:None,last_error:None,request_delay_ms:0,max_retries:0,max_results:0,page_size:0,max_calls:0,hydration_limit:0};
 sqlx::query("INSERT INTO sources(user_id,id,kind,company,board_token,location,enabled) VALUES($1,$2,$3,$4,$5,$6,true) ON CONFLICT(user_id,id) DO UPDATE SET kind=excluded.kind,company=excluded.company,board_token=excluded.board_token,enabled=true,updated_at=now()")
  .bind(user).bind(&source.id).bind(&source.kind).bind(&source.company).bind(&source.board_token).bind(&source.location).execute(&state.db).await?;
 Ok(source)
}

/// Company-directory sources are site-wide administrative configuration. Keep
/// one equivalent source in every account while retaining per-user refresh and
/// job history rows.
pub async fn add_source_for_all(state:&AppState,actor:uuid::Uuid,id:&str)->Result<Source,ApiError>{
 let company:String=sqlx::query_scalar("SELECT company_name FROM company_directory WHERE id=$1").bind(id).fetch_optional(&state.db).await?.ok_or(ApiError::NotFound)?;
 sqlx::query("UPDATE company_directory SET source_enabled=true,updated_at=now() WHERE id=$1").bind(id).execute(&state.db).await?;
 sqlx::query("UPDATE jobs SET active=false WHERE source_id IN(SELECT s.id FROM sources s WHERE s.user_id=jobs.user_id AND lower(btrim(s.company))=lower(btrim($1)))").bind(&company).execute(&state.db).await?;
 sqlx::query("DELETE FROM sources WHERE lower(btrim(company))=lower(btrim($1))").bind(&company).execute(&state.db).await?;
 let users:Vec<uuid::Uuid>=sqlx::query_scalar("SELECT id FROM users ORDER BY id").fetch_all(&state.db).await?;let mut actor_source=None;
 for user in users{let source=add_source(state,user,id).await?;if user==actor{actor_source=Some(source);}}
 actor_source.ok_or(ApiError::NotFound)
}

pub async fn sync_enabled_sources_for_user(state:&AppState,user:uuid::Uuid)->Result<(),ApiError>{
 let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM company_directory WHERE enabled=true AND source_enabled=true ORDER BY id").fetch_all(&state.db).await?;
 for id in ids{add_source(state,user,&id).await?;}
 Ok(())
}

pub async fn remove_sources(state:&AppState,user:uuid::Uuid,id:&str)->Result<usize,ApiError>{
 let entry:CompanyDirectoryEntry=sqlx::query_as("SELECT id,true enabled,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,current_job_count,last_verified_at,verification_status,verification_error,identifier_origin,detected_provider,redirect_url FROM company_directory WHERE id=$1").bind(id).fetch_optional(&state.db).await?.ok_or_else(||ApiError::BadRequest("Company directory entry was not found.".into()))?;
 let kind=if entry.identifier_origin=="Automatic"{entry.detected_provider.clone().unwrap_or(entry.ats_provider.clone())}else{entry.ats_provider.clone()};let board_token=match kind.as_str(){"Workday"|"CareerPage"=>entry.careers_url.clone(),_=>entry.board_identifier.clone()};let existing:Vec<Source>=sqlx::query_as("SELECT id,kind,company,board_token,location,enabled,last_refreshed_at,last_error,request_delay_ms,max_retries,max_results,page_size,max_calls,hydration_limit FROM sources WHERE user_id=$1").bind(user).fetch_all(&state.db).await?;let ids=existing.into_iter().filter(|source|same_identity(&source.company,&entry.company_name)&&source.kind==kind&&same_identity(&source.board_token,&board_token)).map(|source|source.id).collect::<Vec<_>>();
 for source_id in &ids{sqlx::query("DELETE FROM sources WHERE user_id=$1 AND id=$2").bind(user).bind(source_id).execute(&state.db).await?;}
 Ok(ids.len())
}

pub async fn remove_sources_for_all(state:&AppState,id:&str)->Result<usize,ApiError>{
 sqlx::query("UPDATE company_directory SET source_enabled=false,updated_at=now() WHERE id=$1").bind(id).execute(&state.db).await?;
 let users:Vec<uuid::Uuid>=sqlx::query_scalar("SELECT id FROM users ORDER BY id").fetch_all(&state.db).await?;let mut removed=0usize;
 for user in users{removed+=remove_sources(state,user,id).await?;}
 Ok(removed)
}

pub async fn set_company_enabled(state:&AppState,user:uuid::Uuid,id:&str,enabled:bool)->Result<CompanyDirectoryEntry,ApiError>{
 let changed=sqlx::query("UPDATE company_directory SET enabled=$2,updated_at=now() WHERE id=$1").bind(id).bind(enabled).execute(&state.db).await?;if changed.rows_affected()==0{return Err(ApiError::BadRequest("Company directory entry was not found.".into()));}
 entries(state,user).await?.into_iter().find(|entry|entry.id==id).ok_or(ApiError::NotFound)
}

fn same_identity(left:&str,right:&str)->bool{left.trim().trim_end_matches('/').eq_ignore_ascii_case(right.trim().trim_end_matches('/'))}

fn verification_source(entry:&CompanyDirectoryEntry)->Source{
 let kind=if entry.identifier_origin=="Automatic"{entry.detected_provider.clone().unwrap_or_else(||entry.ats_provider.clone())}else{entry.ats_provider.clone()};
 let board_token=match kind.as_str(){"Workday"|"CareerPage"=>entry.careers_url.clone(),_=>entry.board_identifier.clone()};
 Source{id:format!("verify-{}",entry.id),kind,company:entry.company_name.clone(),board_token,location:None,enabled:true,last_refreshed_at:None,last_error:None,request_delay_ms:0,max_retries:0,max_results:0,page_size:0,max_calls:0,hydration_limit:0}
}

pub async fn verify_entry(state:&AppState,id:&str)->Result<CompanyDirectoryEntry,ApiError>{
 let entry:CompanyDirectoryEntry=sqlx::query_as("SELECT id,true enabled,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,current_job_count,last_verified_at,verification_status,verification_error,identifier_origin,detected_provider,redirect_url FROM company_directory WHERE id=$1").bind(id).fetch_optional(&state.db).await?.ok_or_else(||ApiError::BadRequest("Company directory entry was not found.".into()))?;
 let source=verification_source(&entry);
 match connectors::fetch_with_metadata(state,&source,None).await{
  Ok(result)=>{
   let detected=connectors::provider_from_url(&result.final_url).map(str::to_string);
   let provider_changed=detected.as_deref().is_some_and(|provider|provider!=entry.ats_provider);
   let redirected=detected.as_deref()!=connectors::provider_from_url(&entry.careers_url);
   let status=if result.warning.is_some(){"Partial"}else if provider_changed{"Provider change"}else if redirected{"Redirected"}else{"Verified"};
   sqlx::query("UPDATE company_directory SET current_job_count=$2,last_verified_at=now(),next_verification_at=now()+interval '24 hours',verification_status=$3,verification_error=$4,detected_provider=$5,redirect_url=$6,updated_at=now() WHERE id=$1")
    .bind(id).bind(result.reported_count.unwrap_or(result.postings.len()as i32)).bind(status).bind(result.warning).bind(detected).bind(if redirected{Some(result.final_url)}else{None}).execute(&state.db).await?;
  },
  Err(error)=>{sqlx::query("UPDATE company_directory SET last_verified_at=now(),next_verification_at=now()+interval '6 hours',verification_status='Stale',verification_error=$2,updated_at=now() WHERE id=$1").bind(id).bind(error.to_string()).execute(&state.db).await?;}
 }
 Ok(sqlx::query_as("SELECT id,true enabled,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,current_job_count,last_verified_at,verification_status,verification_error,identifier_origin,detected_provider,redirect_url FROM company_directory WHERE id=$1").bind(id).fetch_one(&state.db).await?)
}

pub async fn verify_due(state:&AppState)->Result<usize,ApiError>{
 discover_saved_sources(state).await?;
 let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM company_directory WHERE next_verification_at<=now() ORDER BY next_verification_at LIMIT 3").fetch_all(&state.db).await?;
 for id in &ids{if let Err(error)=verify_entry(state,id).await{tracing::warn!(%id,%error,"company directory verification failed");}}
 Ok(ids.len())
}

fn slug(value:&str)->String{let result:String=value.to_lowercase().chars().map(|character|if character.is_ascii_alphanumeric(){character}else{'-'}).collect();result.split('-').filter(|part|!part.is_empty()).collect::<Vec<_>>().join("-")}
fn careers_url(kind:&str,token:&str)->Option<String>{match kind{
 "Greenhouse"=>Some(format!("https://boards.greenhouse.io/{token}")),"Lever"=>Some(format!("https://jobs.lever.co/{token}")),"Ashby"=>Some(format!("https://jobs.ashbyhq.com/{token}")),
 "SmartRecruiters"=>Some(format!("https://jobs.smartrecruiters.com/{token}")),"Amazon"=>Some("https://www.amazon.jobs/en/search".into()),
 "Microsoft"=>Some("https://apply.careers.microsoft.com/careers?domain=microsoft.com".into()),"Google"=>Some("https://www.google.com/about/careers/applications/jobs/results".into()),
 "Workday"|"CareerPage" if token.starts_with("https://")=>Some(token.into()),_=>None}}

/// Saved sources are treated as automatic discoveries. Their records remain
/// separate from curated/manual rows, so verification can never overwrite a
/// manually confirmed board identifier.
async fn discover_saved_sources(state:&AppState)->Result<(),ApiError>{
 let rows:Vec<(String,String,String,Option<String>)>=sqlx::query_as("SELECT DISTINCT ON(lower(btrim(company))) kind,company,board_token,location FROM sources WHERE enabled=true AND kind NOT IN ('USAJOBS','Adzuna') ORDER BY lower(btrim(company)),(last_error IS NULL) DESC,last_refreshed_at DESC NULLS LAST,CASE kind WHEN 'Greenhouse' THEN 0 WHEN 'Lever' THEN 1 WHEN 'Ashby' THEN 2 WHEN 'SmartRecruiters' THEN 3 WHEN 'Amazon' THEN 4 WHEN 'Workday' THEN 5 WHEN 'Microsoft' THEN 6 WHEN 'Google' THEN 7 ELSE 8 END,id").fetch_all(&state.db).await?;
 for(kind,company,token,location)in rows{
  let Some(url)=careers_url(&kind,&token)else{continue};
  let countries=match location.as_deref(){Some(value)if value.to_lowercase().contains("canada")=>serde_json::json!(["Canada"]),Some(value)if value.to_lowercase().contains("united states")||value.to_lowercase().contains("usa")=>serde_json::json!(["United States"]),_=>serde_json::json!([])};
  let id=format!("auto-{}-{}",slug(&kind),slug(&company));
  let represented:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM company_directory WHERE lower(btrim(company_name))=lower(btrim($1)) AND id<>$2)").bind(&company).bind(&id).fetch_one(&state.db).await?;if represented{continue;}
  sqlx::query("INSERT INTO company_directory(id,company_name,countries,ats_provider,connector_class,board_identifier,careers_url,identifier_origin) VALUES($1,$2,$3,$4,$5,$6,$7,'Automatic') ON CONFLICT(id) DO UPDATE SET company_name=excluded.company_name,countries=excluded.countries,ats_provider=excluded.ats_provider,connector_class=excluded.connector_class,board_identifier=excluded.board_identifier,careers_url=excluded.careers_url,updated_at=now() WHERE company_directory.identifier_origin='Automatic'")
   .bind(id).bind(company).bind(countries).bind(&kind).bind(connectors::connector_class(&kind)).bind(token).bind(url).execute(&state.db).await?;
 }
 Ok(())
}

#[cfg(test)]mod tests{use super::*;#[test]fn duplicate_identity_ignores_case_whitespace_and_trailing_slashes(){assert!(same_identity(" Example ","example"));assert!(same_identity("https://example.com/jobs/","https://example.com/jobs"));assert!(!same_identity("example","another"));}}
