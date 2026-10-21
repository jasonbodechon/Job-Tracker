use regex::Regex;
use serde_json::{json, Map, Value};
use sha2::{Digest,Sha256};
use tokio::time::{sleep,Duration};
use url::Url;

use crate::{crypto, error::ApiError, models::{Posting, Source}, AppState};

#[derive(Debug)]
pub struct FetchResult { pub postings:Vec<Posting>, pub final_url:String, pub reported_count:Option<i32>, pub warning:Option<String> }

#[derive(Debug,Clone,Copy,PartialEq)]
pub struct ConnectorSettings{pub request_delay_ms:usize,pub max_retries:usize,pub max_results:usize,pub page_size:usize,pub max_calls:usize,pub hydration_limit:usize}

pub fn default_settings(kind:&str)->ConnectorSettings{match kind{
 "Amazon"=>ConnectorSettings{request_delay_ms:250,max_retries:4,max_results:1000,page_size:100,max_calls:10,hydration_limit:0},
 "USAJOBS"=>ConnectorSettings{request_delay_ms:100,max_retries:3,max_results:2500,page_size:250,max_calls:10,hydration_limit:0},
 "SmartRecruiters"=>ConnectorSettings{request_delay_ms:125,max_retries:3,max_results:2000,page_size:100,max_calls:20,hydration_limit:0},
 "Workday"=>ConnectorSettings{request_delay_ms:250,max_retries:3,max_results:1000,page_size:20,max_calls:50,hydration_limit:0},
 "Adzuna"=>ConnectorSettings{request_delay_ms:250,max_retries:3,max_results:1000,page_size:20,max_calls:50,hydration_limit:0},
 "Google"=>ConnectorSettings{request_delay_ms:250,max_retries:3,max_results:1000,page_size:20,max_calls:50,hydration_limit:0},
 "Microsoft"=>ConnectorSettings{request_delay_ms:1000,max_retries:4,max_results:1000,page_size:10,max_calls:100,hydration_limit:50},
 _=>ConnectorSettings{request_delay_ms:0,max_retries:3,max_results:5000,page_size:100,max_calls:1,hydration_limit:0}}}

fn settings(state:&AppState,source:&Source)->ConnectorSettings{let defaults=default_settings(&source.kind);let verification=source.id.starts_with("verify-");ConnectorSettings{
 request_delay_ms:if source.request_delay_ms>0{source.request_delay_ms as usize}else{defaults.request_delay_ms},
 max_retries:if source.max_retries>0{source.max_retries as usize}else{defaults.max_retries},
 max_results:if verification{100}else{(if source.max_results>0{source.max_results as usize}else{defaults.max_results}).min(state.config.connector_max_results)},
 page_size:if verification{100}else{if source.page_size>0{source.page_size as usize}else{defaults.page_size}},
 max_calls:if verification{1}else{if source.max_calls>0{source.max_calls as usize}else{defaults.max_calls}},
 hydration_limit:if verification{0}else{if source.hydration_limit>0{source.hydration_limit as usize}else{defaults.hydration_limit}},}}

fn failure(source:&Source,code:&str,description:impl AsRef<str>)->ApiError{ApiError::BadRequest(format!("{} connector [{}]: {}",source.kind,code,description.as_ref()))}
fn public_url(value:&str)->String{Url::parse(value).map(|mut url|{let pairs=url.query_pairs().map(|(key,value)|{let sensitive=matches!(key.as_ref(),"app_key"|"app_id"|"api_key"|"key"|"token");(key.into_owned(),if sensitive{"REDACTED".into()}else{value.into_owned()})}).collect::<Vec<_>>();url.set_query(None);if !pairs.is_empty(){let mut query=url.query_pairs_mut();for(key,value)in pairs{query.append_pair(&key,&value);}}url.to_string()}).unwrap_or_else(|_|"invalid URL".into())}
fn body_excerpt(value:&str)->String{value.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect()}
async fn delay(settings:ConnectorSettings,calls:usize){if calls>0&&settings.request_delay_ms>0{sleep(Duration::from_millis(settings.request_delay_ms as u64)).await;}}
async fn request_text(source:&Source,request:reqwest::RequestBuilder,settings:ConnectorSettings)->Result<(String,String),ApiError>{
 let attempts=settings.max_retries+1;for attempt in 1..=attempts{let Some(next)=request.try_clone()else{return Err(failure(source,"REQUEST_NOT_CLONEABLE","The request could not be retried safely."));};match next.send().await{
  Ok(response)=>{let status=response.status();let url=response.url().to_string();let retry_after=response.headers().get(reqwest::header::RETRY_AFTER).and_then(|value|value.to_str().ok()).and_then(|value|value.parse::<u64>().ok());let body=response.text().await.map_err(|error|failure(source,"RESPONSE_READ",error.to_string()))?;if status.is_success(){return Ok((body,url));}let retryable=status.as_u16()==429||status.is_server_error();if retryable&&attempt<attempts{let backoff=retry_after.map(|seconds|seconds*1000).unwrap_or_else(||((settings.request_delay_ms.max(250)as u64)*2u64.pow((attempt-1)as u32)).min(30000));sleep(Duration::from_millis(backoff)).await;continue;}return Err(failure(source,&format!("HTTP_{}",status.as_u16()),format!("{} from {} after {attempt}/{attempts} attempts. Response: {}",status.canonical_reason().unwrap_or("HTTP error"),public_url(&url),body_excerpt(&body))));},
  Err(error)=>{if attempt<attempts&&(error.is_timeout()||error.is_connect()||error.is_request()){let backoff=((settings.request_delay_ms.max(250)as u64)*2u64.pow((attempt-1)as u32)).min(30000);sleep(Duration::from_millis(backoff)).await;continue;}let code=if error.is_timeout(){"NETWORK_TIMEOUT"}else if error.is_connect(){"NETWORK_CONNECT"}else{"NETWORK_REQUEST"};return Err(failure(source,code,format!("Request failed after {attempt}/{attempts} attempts: {error}")));}
 }}unreachable!()
}

fn text(value:&Value,key:&str)->String { value.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string() }
fn first_text(value:&Value,keys:&[&str])->String { keys.iter().map(|key|text(value,key)).find(|value|!value.is_empty()).unwrap_or_default() }
fn encoded(value:&str)->String { url::form_urlencoded::byte_serialize(value.as_bytes()).collect() }
fn needs_next_page(loaded:usize,page_size:usize,reported_count:Option<i32>,limit:usize,requested_page_size:usize)->bool{
 if page_size==0||loaded>=limit{return false;}
 reported_count.map(|total|loaded<(total.max(0)as usize).min(limit)).unwrap_or(page_size>=requested_page_size)
}
fn retain_first_url(target:&mut Option<String>,response_url:String){if target.is_none(){*target=Some(response_url);}}

fn strip_html(value:&str)->String {
 let mut out=String::new();let mut tag=false;
 for character in value.chars(){match character{'<'=>tag=true,'>'=>{tag=false;out.push(' ');},_ if !tag=>out.push(character),_=>{}}}
 out.replace("&nbsp;"," ").replace("&#160;"," ").replace("&amp;","&").replace("&quot;","\"").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn remote_type(location:&str,description:&str,explicit:&str)->String {
 let location_value=location.to_lowercase();let value=format!("{explicit} {location} {description}").to_lowercase();
 if location_value.contains("hybrid")||value.contains("hybrid") {"Hybrid".into()}
 else if location_value.contains("remote")||value.contains("fully remote")||value.contains("100% remote")||explicit.eq_ignore_ascii_case("remote") {"Remote".into()}
 else if location.trim().is_empty(){"Unknown".into()}else{"On-site".into()}
}

fn safe_public_url(value:&str)->Result<Url,ApiError>{
 let parsed=Url::parse(value).map_err(|_|ApiError::BadRequest("Enter a valid HTTPS career-site URL.".into()))?;
 if parsed.scheme()!="https"{return Err(ApiError::BadRequest("Career-site connectors require HTTPS.".into()));}
 let host=parsed.host_str().unwrap_or_default().to_lowercase();
 if host.is_empty()||host=="localhost"||host.ends_with(".local")||host.parse::<std::net::IpAddr>().is_ok(){return Err(ApiError::BadRequest("Career-site URLs must use a public hostname.".into()));}
 Ok(parsed)
}

async fn get_json(state:&AppState,source:&Source,url:&str,headers:&[(&str,String)],settings:ConnectorSettings)->Result<(Value,String),ApiError>{
 let mut request=state.client.get(url);for(key,value)in headers{request=request.header(*key,value);}
 let(body,final_url)=request_text(source,request,settings).await?;let data=serde_json::from_str(&body).map_err(|error|failure(source,"INVALID_JSON",format!("The response was not valid JSON: {error}. Response: {}",body_excerpt(&body))))?;Ok((data,final_url))
}
async fn get_html(state:&AppState,source:&Source,url:&str,settings:ConnectorSettings)->Result<(String,String),ApiError>{
 safe_public_url(url).map_err(|error|failure(source,"INVALID_PUBLIC_URL",error.to_string()))?;request_text(source,state.client.get(url).header(reqwest::header::ACCEPT,"text/html,application/xhtml+xml").header(reqwest::header::USER_AGENT,"Mozilla/5.0 (compatible; Job-Tracker/0.18; +https://localhost)"),settings).await
}

pub fn connector_class(provider:&str)->&'static str{match provider{
 "Greenhouse"|"Lever"|"Ashby"|"SmartRecruiters"|"Adzuna"|"USAJOBS"=>"Official API",
 "Amazon"|"Workday"|"Microsoft"=>"Public site feed",_=>"Public page extraction"}}

pub fn provider_from_url(value:&str)->Option<&'static str>{let host=Url::parse(value).ok()?.host_str()?.to_lowercase();
 if host.contains("greenhouse.io"){Some("Greenhouse")}else if host.contains("lever.co"){Some("Lever")}
 else if host.contains("ashbyhq.com"){Some("Ashby")}else if host.contains("smartrecruiters.com"){Some("SmartRecruiters")}
 else if host.contains("myworkdayjobs.com"){Some("Workday")}else if host.contains("amazon.jobs"){Some("Amazon")}
 else if host.contains("careers.microsoft.com"){Some("Microsoft")}else if host.contains("google.com")&&value.contains("careers"){Some("Google")}else{None}}

pub async fn fetch(state:&AppState,source:&Source,encrypted:Option<&str>)->Result<Vec<Posting>,ApiError>{Ok(fetch_with_metadata(state,source,encrypted).await?.postings)}
pub async fn fetch_with_metadata(state:&AppState,source:&Source,encrypted:Option<&str>)->Result<FetchResult,ApiError>{match source.kind.as_str(){
 "Greenhouse"=>fetch_greenhouse(state,source).await,"Lever"=>fetch_lever(state,source).await,"Ashby"=>fetch_ashby(state,source).await,
 "USAJOBS"=>fetch_usajobs(state,source,encrypted).await,"Amazon"=>fetch_amazon(state,source).await,"SmartRecruiters"=>fetch_smartrecruiters(state,source).await,
 "Workday"=>fetch_workday(state,source).await,"Microsoft"=>fetch_microsoft(state,source).await,
 "Google"=>fetch_google(state,source).await,
 "CareerPage"=>fetch_public_page(state,source,&source.board_token).await,"Adzuna"=>fetch_adzuna(state,source).await,
 _=>Err(ApiError::BadRequest("Unsupported source provider.".into()))}}

async fn fetch_greenhouse(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let url=format!("https://boards-api.greenhouse.io/v1/boards/{}/jobs?content=true",encoded(&source.board_token));let(data,final_url)=get_json(state,source,&url,&[],settings(state,source)).await?;
 let postings=data.get("jobs").and_then(Value::as_array).into_iter().flatten().map(|job|{let location=job.pointer("/location/name").and_then(Value::as_str).unwrap_or_default();let description=strip_html(&text(job,"content"));Posting{external_id:job.get("id").map(Value::to_string).unwrap_or_default(),title:text(job,"title"),company:source.company.clone(),location:location.into(),remote_type:remote_type(location,&description,""),posted_at:text(job,"updated_at"),apply_url:text(job,"absolute_url"),description}}).collect::<Vec<_>>();
 Ok(FetchResult{reported_count:Some(postings.len()as i32),postings,final_url,warning:None})}

async fn fetch_lever(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let url=format!("https://api.lever.co/v0/postings/{}?mode=json",encoded(&source.board_token));let(data,final_url)=get_json(state,source,&url,&[],settings(state,source)).await?;
 let postings=data.as_array().into_iter().flatten().map(|job|{let location=job.pointer("/categories/location").and_then(Value::as_str).unwrap_or_default();let description=strip_html(&format!("{} {}",text(job,"descriptionPlain"),text(job,"additionalPlain")));Posting{external_id:text(job,"id"),title:text(job,"text"),company:source.company.clone(),location:location.into(),remote_type:remote_type(location,&description,&text(job,"workplaceType")),posted_at:job.get("createdAt").and_then(Value::as_i64).and_then(chrono::DateTime::from_timestamp_millis).map(|date|date.to_rfc3339()).unwrap_or_default(),apply_url:first_text(job,&["applyUrl","hostedUrl"]),description}}).collect::<Vec<_>>();
 Ok(FetchResult{reported_count:Some(postings.len()as i32),postings,final_url,warning:None})}

async fn fetch_ashby(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let url=format!("https://api.ashbyhq.com/posting-api/job-board/{}",encoded(&source.board_token));let(data,final_url)=get_json(state,source,&url,&[],settings(state,source)).await?;
 let postings=data.get("jobs").and_then(Value::as_array).into_iter().flatten().map(|job|{let location=text(job,"location");let description=strip_html(&first_text(job,&["descriptionPlain","descriptionHtml","description"]));let explicit=if job.get("isRemote").and_then(Value::as_bool)==Some(true){"Remote".into()}else{text(job,"workplaceType")};Posting{external_id:first_text(job,&["id","jobUrl"]),title:text(job,"title"),company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,&explicit),posted_at:text(job,"publishedAt"),apply_url:first_text(job,&["applyUrl","jobUrl"]),description}}).collect::<Vec<_>>();
 Ok(FetchResult{reported_count:Some(postings.len()as i32),postings,final_url,warning:None})}

async fn fetch_usajobs(state:&AppState,source:&Source,encrypted:Option<&str>)->Result<FetchResult,ApiError>{
 let raw=encrypted.ok_or_else(||failure(source,"MISSING_CREDENTIALS","USAJOBS credentials are missing."))?;let decoded=crypto::decrypt(&state.config.encryption_key,raw).map_err(ApiError::Internal)?;let credentials:Value=serde_json::from_slice(&decoded)?;
 let headers=[("User-Agent",text(&credentials,"email")),("Authorization-Key",text(&credentials,"apiKey"))];let tuning=settings(state,source);let limit=tuning.max_results;let mut page=1usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let url=format!("https://data.usajobs.gov/api/search?Keyword={}&LocationName={}&ResultsPerPage={}&Page={page}",encoded(&source.board_token),encoded(source.location.as_deref().unwrap_or("")),tuning.page_size);let response=get_json(state,source,&url,&headers,tuning).await;let(data,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};calls+=1;retain_first_url(&mut final_url,response_url);if reported_count.is_none(){reported_count=data.pointer("/SearchResult/SearchResultCountAll").and_then(Value::as_i64).map(|value|value as i32);}let rows=data.pointer("/SearchResult/SearchResultItems").and_then(Value::as_array);let page_size=rows.map(Vec::len).unwrap_or(0);
  postings.extend(rows.into_iter().flatten().map(|job|{let descriptor=job.get("MatchedObjectDescriptor").unwrap_or(job);let location=descriptor.get("PositionLocationDisplay").and_then(Value::as_str).unwrap_or_default();let details=descriptor.pointer("/UserArea/Details").unwrap_or(&Value::Null);let description=strip_html(&[text(details,"JobSummary"),text(details,"Requirements"),text(details,"Qualifications")].join(" "));Posting{external_id:text(descriptor,"PositionID"),title:text(descriptor,"PositionTitle"),company:text(descriptor,"OrganizationName"),location:location.into(),remote_type:remote_type(location,&description,if details.get("RemoteIndicator").and_then(Value::as_bool)==Some(true){"Remote"}else{""}),posted_at:text(descriptor,"PublicationStartDate"),apply_url:text(descriptor,"PositionURI"),description}}));
  if !needs_next_page(postings.len(),page_size,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs.",calls,postings.len()));}break;}page+=1;
 }
 postings.truncate(limit);let postings=deduplicate(postings);let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","USAJOBS did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})}

async fn fetch_amazon(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let query=source.location.as_deref().filter(|value|!value.trim().is_empty()).unwrap_or_else(||if source.board_token.eq_ignore_ascii_case("amazon"){""}else{source.board_token.as_str()});let tuning=settings(state,source);let limit=tuning.max_results;let mut offset=0usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let url=format!("https://www.amazon.jobs/en/search.json?base_query={}&normalized_country_code%5B%5D=CAN&normalized_country_code%5B%5D=USA&offset={offset}&result_limit={}&sort=recent",encoded(query),tuning.page_size);let response=get_json(state,source,&url,&[],tuning).await;let(data,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later Amazon page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};calls+=1;retain_first_url(&mut final_url,response_url);if reported_count.is_none(){reported_count=data.get("hits").and_then(Value::as_i64).map(|value|value as i32);}let rows=data.get("jobs").and_then(Value::as_array).ok_or_else(||failure(source,"SCHEMA_MISSING_JOBS","Amazon returned JSON without the expected jobs array; its public feed may have changed."))?;let page_count=rows.len();postings.extend(rows.iter().map(|job|normalize_amazon(job,source)));if !needs_next_page(postings.len(),page_count,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs; increase Max calls to retrieve more.",calls,postings.len()));}break;}offset+=page_count;}
 postings.truncate(limit);let postings=deduplicate(postings);let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","Amazon did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})}
fn normalize_amazon(job:&Value,source:&Source)->Posting{let location=first_text(job,&["location","location_name","normalized_location"]);let description=strip_html(&first_text(job,&["description","description_short","basic_qualifications"]));let path=first_text(job,&["job_path","url","apply_url"]);let apply_url=if path.starts_with("http"){path}else{format!("https://www.amazon.jobs{path}")};Posting{external_id:first_text(job,&["id","job_id"]),title:first_text(job,&["title","name"]),company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,&text(job,"workplace_type")),posted_at:first_text(job,&["posted_date","updated_at"]),apply_url,description}}

async fn fetch_smartrecruiters(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let tuning=settings(state,source);let limit=tuning.max_results;let mut offset=0usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let url=format!("https://api.smartrecruiters.com/v1/companies/{}/postings?limit={}&offset={offset}",encoded(&source.board_token),tuning.page_size);let response=get_json(state,source,&url,&[],tuning).await;let(data,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};calls+=1;retain_first_url(&mut final_url,response_url);if reported_count.is_none(){reported_count=data.get("totalFound").or_else(||data.get("total")).and_then(Value::as_i64).map(|value|value as i32);}let rows=data.get("content").or_else(||data.get("postings")).and_then(Value::as_array);let page_size=rows.map(Vec::len).unwrap_or(0);
  postings.extend(rows.into_iter().flatten().map(|job|{let location_value=job.get("location").unwrap_or(&Value::Null);let location=[text(location_value,"city"),text(location_value,"region"),text(location_value,"country")].into_iter().filter(|value|!value.is_empty()).collect::<Vec<_>>().join(", ");let description=strip_html(job.pointer("/jobAd/sections/jobDescription/text").and_then(Value::as_str).unwrap_or_else(||job.get("description").and_then(Value::as_str).unwrap_or_default()));Posting{external_id:text(job,"id"),title:first_text(job,&["name","title"]),company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,&text(job,"typeOfEmployment")),posted_at:first_text(job,&["releasedDate","createdOn"]),apply_url:first_text(job,&["ref","applyUrl"]),description}}));
  if !needs_next_page(postings.len(),page_size,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs.",calls,postings.len()));}break;}offset+=page_size;
 }
 postings.truncate(limit);let postings=deduplicate(postings);let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","SmartRecruiters did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})}

async fn fetch_workday(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let board=safe_public_url(&source.board_token).map_err(|error|failure(source,"INVALID_WORKDAY_URL",error.to_string()))?;let host=board.host_str().unwrap_or_default();if !host.ends_with("myworkdayjobs.com"){return Err(failure(source,"INVALID_WORKDAY_HOST","Workday board identifier must be a public myworkdayjobs.com career-site URL."));}let tenant=host.split('.').next().unwrap_or_default();let site=board.path().trim_matches('/').split('/').next_back().unwrap_or_default();if tenant.is_empty()||site.is_empty(){return Err(failure(source,"INVALID_WORKDAY_SITE","Workday URL must include its tenant and career-site name."));}
 let url=format!("https://{host}/wday/cxs/{tenant}/{site}/jobs");let tuning=settings(state,source);let limit=tuning.max_results;let mut offset=0usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let request=state.client.post(&url).json(&json!({"appliedFacets":{},"limit":tuning.page_size,"offset":offset,"searchText":source.location.as_deref().unwrap_or("")}));let response=request_text(source,request,tuning).await;let(body,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};let data:Value=serde_json::from_str(&body).map_err(|error|failure(source,"INVALID_JSON",format!("The Workday response was not valid JSON: {error}. Response: {}",body_excerpt(&body))))?;calls+=1;retain_first_url(&mut final_url,response_url);if reported_count.is_none(){reported_count=data.get("total").and_then(Value::as_i64).map(|value|value as i32);}let rows=data.get("jobPostings").and_then(Value::as_array);let page_size=rows.map(Vec::len).unwrap_or(0);
  postings.extend(rows.into_iter().flatten().map(|job|{let path=text(job,"externalPath");let apply_url=board.join(&path).map(|url|url.to_string()).unwrap_or_else(|_|source.board_token.clone());let location=first_text(job,&["locationsText","location"]);let description=job.get("bulletFields").and_then(Value::as_array).map(|items|items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default();Posting{external_id:first_text(job,&["externalPath","id"]),title:text(job,"title"),company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,""),posted_at:text(job,"postedOn"),apply_url,description}}));
  if !needs_next_page(postings.len(),page_size,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs.",calls,postings.len()));}break;}offset+=page_size;
 }
 postings.truncate(limit);let postings=deduplicate(postings);let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","Workday did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})}

async fn fetch_adzuna(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let app_id=state.config.adzuna_app_id.as_deref().ok_or_else(||failure(source,"MISSING_APP_ID","ADZUNA_APP_ID is not configured."))?;let app_key=state.config.adzuna_app_key.as_deref().ok_or_else(||failure(source,"MISSING_APP_KEY","ADZUNA_APP_KEY is not configured."))?;let country=if source.location.as_deref().map(|value|value.eq_ignore_ascii_case("Canada")).unwrap_or(false){"ca"}else{"us"};
 let tuning=settings(state,source);let limit=tuning.max_results;let mut page=1usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let url=format!("https://api.adzuna.com/v1/api/jobs/{country}/search/{page}?app_id={}&app_key={}&results_per_page={}&what={}",encoded(app_id),encoded(app_key),tuning.page_size,encoded(&source.board_token));let response=get_json(state,source,&url,&[],tuning).await;let(data,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};calls+=1;retain_first_url(&mut final_url,response_url);if reported_count.is_none(){reported_count=data.get("count").and_then(Value::as_i64).map(|value|value as i32);}let rows=data.get("results").and_then(Value::as_array);let page_size=rows.map(Vec::len).unwrap_or(0);
  postings.extend(rows.into_iter().flatten().map(|job|{let location=job.pointer("/location/display_name").and_then(Value::as_str).unwrap_or_default().to_string();let description=strip_html(&text(job,"description"));Posting{external_id:first_text(job,&["id","redirect_url"]),title:text(job,"title"),company:job.pointer("/company/display_name").and_then(Value::as_str).unwrap_or(&source.company).to_string(),location:location.clone(),remote_type:remote_type(&location,&description,""),posted_at:text(job,"created"),apply_url:text(job,"redirect_url"),description}}));
  if !needs_next_page(postings.len(),page_size,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs.",calls,postings.len()));}break;}page+=1;
 }
 postings.truncate(limit);let postings=deduplicate(postings);let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","Adzuna did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})}

async fn fetch_microsoft(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let tuning=settings(state,source);
 let requested=tuning.page_size.clamp(1,10);
 let limit=tuning.max_results;
 let query=if source.board_token.eq_ignore_ascii_case("microsoft"){""}else{source.board_token.as_str()};
 let mut start=0usize;
 let mut calls=0usize;
 let mut summaries=Vec::new();
 let mut reported_count=None;
 let mut final_url=None;
 let mut warning=None;
 loop{
  delay(tuning,calls).await;
  let url=format!("https://microsoft.eightfold.ai/api/pcsx/search?domain=microsoft.com&query={}&location={}&start={start}&num={requested}",encoded(query),encoded(source.location.as_deref().unwrap_or("")));
  let response=get_json(state,source,&url,&[],tuning).await;
  let(envelope,response_url)=match response{Ok(value)=>value,Err(error)if !summaries.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later Microsoft page failed. {}",summaries.len(),error));break;},Err(error)=>return Err(error)};
  calls+=1;
  retain_first_url(&mut final_url,response_url);
  let payload=microsoft_payload(&envelope,source,"search")?;
  let count=payload.get("count").or_else(||payload.get("totalJobs")).or_else(||payload.get("total")).and_then(Value::as_i64).ok_or_else(||failure(source,"MICROSOFT_SEARCH_CONTRACT_CHANGED","The search response no longer contains a numeric job count."))?;
  if reported_count.is_none(){reported_count=Some(count as i32);}
  let rows=payload.get("positions").or_else(||payload.get("jobs")).and_then(Value::as_array).ok_or_else(||failure(source,"MICROSOFT_SEARCH_CONTRACT_CHANGED","The search response no longer contains the expected positions array."))?;
  for row in rows{validate_microsoft_summary(row,source)?;}
  let page_size=rows.len();
  summaries.extend(rows.iter().cloned());
  if !needs_next_page(summaries.len(),page_size,reported_count,limit,requested){break;}
  if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} Microsoft jobs.",calls,summaries.len()));}break;}
  start+=page_size;
 }
 summaries.truncate(limit);
 let mut seen=std::collections::HashSet::new();
 summaries.retain(|summary|seen.insert(microsoft_external_id(summary)));
 let mut postings=Vec::with_capacity(summaries.len());
 let mut hydration_attempts=0usize;
 let mut hydration_failures=0usize;
 let mut first_hydration_error=None;
 for summary in summaries{
  let external_id=microsoft_external_id(&summary);
  let fingerprint=microsoft_summary_fingerprint(&summary);
  let cached:Option<(String,Value)>=sqlx::query_as("SELECT summary_fingerprint,detail FROM connector_detail_cache WHERE provider='Microsoft' AND external_id=$1").bind(&external_id).fetch_optional(&state.db).await?;
  let cache_matches=cached.as_ref().is_some_and(|(stored,_)|stored==&fingerprint);
  let mut detail=cached.as_ref().map(|(_,value)|value.clone());
  if cache_matches{
   sqlx::query("UPDATE connector_detail_cache SET last_used_at=now() WHERE provider='Microsoft' AND external_id=$1").bind(&external_id).execute(&state.db).await?;
  }else if hydration_attempts<tuning.hydration_limit{
   hydration_attempts+=1;
   if tuning.request_delay_ms>0{sleep(Duration::from_millis(tuning.request_delay_ms as u64)).await;}
   match fetch_microsoft_detail(state,source,&external_id,tuning).await{
    Ok(value)=>{
     sqlx::query("INSERT INTO connector_detail_cache(provider,external_id,summary_fingerprint,detail,hydrated_at,last_used_at) VALUES('Microsoft',$1,$2,$3,now(),now()) ON CONFLICT(provider,external_id) DO UPDATE SET summary_fingerprint=excluded.summary_fingerprint,detail=excluded.detail,hydrated_at=now(),last_used_at=now()")
      .bind(&external_id).bind(&fingerprint).bind(&value).execute(&state.db).await?;
     detail=Some(value);
    },
    Err(error)=>{hydration_failures+=1;if first_hydration_error.is_none(){first_hydration_error=Some(error.to_string());}}
   }
  }else if cached.is_some(){
   sqlx::query("UPDATE connector_detail_cache SET last_used_at=now() WHERE provider='Microsoft' AND external_id=$1").bind(&external_id).execute(&state.db).await?;
  }
  let combined=detail.as_ref().map(|value|merge_microsoft_detail(&summary,value)).unwrap_or(summary);
  postings.push(normalize_microsoft(&combined,source));
 }
 if hydration_failures>0{
  let hydration_warning=format!("[MICROSOFT_HYDRATION_PARTIAL] {hydration_failures} changed job detail request(s) failed; cached or summary data was retained. {}",first_hydration_error.unwrap_or_default());
  warning=Some(warning.map(|value|format!("{value} {hydration_warning}")).unwrap_or(hydration_warning));
 }
 let reported_count=reported_count.or(Some(postings.len()as i32));
 let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","Microsoft Careers did not return a usable response."))?;
 Ok(FetchResult{postings,final_url,reported_count,warning})
}

fn scalar_text(value:&Value,key:&str)->String{value.get(key).map(|item|match item{Value::String(value)=>value.trim().to_string(),Value::Number(value)=>value.to_string(),_=>String::new()}).unwrap_or_default()}
fn microsoft_payload<'a>(envelope:&'a Value,source:&Source,operation:&str)->Result<&'a Value,ApiError>{
 if let Some(status)=envelope.get("status").and_then(Value::as_i64){if status!=200{return Err(failure(source,"MICROSOFT_UPSTREAM_STATUS",format!("Microsoft {operation} returned upstream status {status}.")));}}
 let upstream_error=envelope.pointer("/error/message").and_then(Value::as_str).unwrap_or_default().trim();
 if !upstream_error.is_empty(){return Err(failure(source,"MICROSOFT_UPSTREAM_ERROR",format!("Microsoft {operation} returned: {upstream_error}")));}
 let payload=envelope.get("data").unwrap_or(envelope);
 if !payload.is_object(){return Err(failure(source,"MICROSOFT_RESPONSE_CONTRACT_CHANGED",format!("Microsoft {operation} no longer returns an object in the data envelope.")));}
 Ok(payload)
}
fn microsoft_external_id(job:&Value)->String{let value=first_text(job,&["id","positionId","displayJobId"]);if value.is_empty(){scalar_text(job,"id")}else{value}}
fn validate_microsoft_summary(job:&Value,source:&Source)->Result<(),ApiError>{
 if microsoft_external_id(job).is_empty()||first_text(job,&["title","name","positionTitle"]).is_empty()||first_text(job,&["positionUrl","position_url","url","applyUrl"]).is_empty(){return Err(failure(source,"MICROSOFT_SEARCH_CONTRACT_CHANGED","A Microsoft search record is missing its ID, title, or position URL."));}
 if !job.get("locations").is_some_and(Value::is_array)&&!job.get("location").is_some_and(Value::is_string){return Err(failure(source,"MICROSOFT_SEARCH_CONTRACT_CHANGED","A Microsoft search record no longer contains a recognized location field."));}
 Ok(())
}
fn microsoft_summary_fingerprint(job:&Value)->String{
 let stable=json!({"id":microsoft_external_id(job),"title":first_text(job,&["title","name","positionTitle"]),"locations":job.get("locations"),"postedTs":job.get("postedTs"),"creationTs":job.get("creationTs"),"workLocationOption":job.get("workLocationOption"),"positionUrl":first_text(job,&["positionUrl","position_url","url","applyUrl"])});
 hex::encode(Sha256::digest(serde_json::to_vec(&stable).unwrap_or_default()))
}
async fn fetch_microsoft_detail(state:&AppState,source:&Source,external_id:&str,tuning:ConnectorSettings)->Result<Value,ApiError>{
 let url=format!("https://microsoft.eightfold.ai/api/pcsx/position_details?position_id={}&domain=microsoft.com",encoded(external_id));
 let(envelope,_)=get_json(state,source,&url,&[],tuning).await?;
 let payload=microsoft_payload(&envelope,source,"position detail")?;
 let detail_id=microsoft_external_id(payload);
 let description=first_text(payload,&["jobDescription","description","descriptionText"]);
 if detail_id!=external_id||description.is_empty(){return Err(failure(source,"MICROSOFT_DETAIL_CONTRACT_CHANGED","The position-detail response is missing the matching ID or full job description."));}
 Ok(payload.clone())
}
fn merge_microsoft_detail(summary:&Value,detail:&Value)->Value{
 let mut merged=summary.as_object().cloned().unwrap_or_default();
 if let Some(values)=detail.as_object(){for(key,value)in values{merged.insert(key.clone(),value.clone());}}
 Value::Object(merged)
}
fn microsoft_location(job:&Value)->String{if let Some(value)=job.get("location").and_then(Value::as_str){return value.trim().to_string();}job.get("locations").and_then(Value::as_array).map(|values|values.iter().filter_map(|value|value.as_str().map(str::to_string).or_else(||value.get("name").and_then(Value::as_str).map(str::to_string))).collect::<Vec<_>>().join("; ")).unwrap_or_default()}
fn microsoft_posted_at(job:&Value)->String{let direct=first_text(job,&["postedAt","datePosted","posted_at"]);if !direct.is_empty(){return direct;}job.get("postedTs").and_then(Value::as_i64).and_then(|value|if value>10_000_000_000{chrono::DateTime::from_timestamp_millis(value)}else{chrono::DateTime::from_timestamp(value,0)}).map(|value|value.to_rfc3339()).unwrap_or_default()}
fn normalize_microsoft(job:&Value,source:&Source)->Posting{let external_id=microsoft_external_id(job);let title=first_text(job,&["title","name","positionTitle"]);let location=microsoft_location(job);let description=strip_html(&first_text(job,&["jobDescription","description","descriptionText","summary"]));let raw_url=first_text(job,&["publicUrl","positionUrl","position_url","url","applyUrl"]);let apply_url=if raw_url.starts_with("http"){raw_url}else if raw_url.is_empty(){format!("https://apply.careers.microsoft.com/careers/job/{external_id}")}else{format!("https://apply.careers.microsoft.com{}{}",if raw_url.starts_with('/'){""}else{"/"},raw_url)};let explicit=first_text(job,&["workLocationOption","workplaceType","workplace_type"]);Posting{external_id,title,company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,&explicit),posted_at:microsoft_posted_at(job),apply_url,description}}

async fn fetch_public_page(state:&AppState,source:&Source,default_url:&str)->Result<FetchResult,ApiError>{
 let target=if source.board_token.starts_with("https://"){source.board_token.as_str()}else{default_url};let(html,final_url)=get_html(state,source,target,settings(state,source)).await?;let mut postings=json_ld_postings(&html,source,&final_url);if postings.is_empty(){postings=embedded_job_postings(&html,source,&final_url);}if postings.is_empty(){return Err(failure(source,"NO_STRUCTURED_JOBS","The public career page returned no structured job postings; its extraction adapter may need updating."));}Ok(FetchResult{reported_count:Some(postings.len()as i32),postings,final_url,warning:None})}

async fn fetch_google(state:&AppState,source:&Source)->Result<FetchResult,ApiError>{
 let base="https://www.google.com/about/careers/applications/jobs/results";let tuning=settings(state,source);let limit=tuning.max_results;let mut page=1usize;let mut calls=0usize;let mut postings=Vec::new();let mut reported_count=None;let mut final_url=None;let mut warning=None;
 loop{delay(tuning,calls).await;let url=if page==1{base.to_string()}else{format!("{base}?page={page}")};let response=get_html(state,source,&url,tuning).await;let(html,response_url)=match response{Ok(value)=>value,Err(error)if !postings.is_empty()=>{warning=Some(format!("[PARTIAL_REFRESH] Kept {} jobs after a later page failed. {}",postings.len(),error));break;},Err(error)=>return Err(error)};calls+=1;retain_first_url(&mut final_url,response_url.clone());if reported_count.is_none(){reported_count=google_reported_count(&html);}let page_postings=google_html_postings(&html,source,&response_url);let page_size=page_postings.len();postings.extend(page_postings);if !needs_next_page(postings.len(),page_size,reported_count,limit,tuning.page_size){break;}if calls>=tuning.max_calls{if !source.id.starts_with("verify-"){warning=Some(format!("[MAX_CALLS_REACHED] Stopped after {} calls with {} jobs.",calls,postings.len()));}break;}page+=1;}
 postings.truncate(limit);let postings=deduplicate(postings);if postings.is_empty(){return Err(failure(source,"EXTRACTOR_NO_JOBS","Google Careers returned no recognizable public job cards; its page extractor may need updating."));}let reported_count=reported_count.or(Some(postings.len()as i32));let final_url=final_url.ok_or_else(||failure(source,"EMPTY_RESPONSE","Google Careers did not return a usable response."))?;Ok(FetchResult{postings,final_url,reported_count,warning})
}

fn google_reported_count(html:&str)->Option<i32>{let patterns=[r"(?i)([0-9][0-9,]*)\s+jobs?\s+matched",r"(?i)showing\s+[0-9][0-9,]*\s+to\s+[0-9][0-9,]*\s+of\s+([0-9][0-9,]*)\s+rows?"];patterns.iter().find_map(|pattern|Regex::new(pattern).unwrap().captures(html).and_then(|captures|captures.get(1)).and_then(|value|value.as_str().replace(',',"").parse().ok()))}
fn google_html_postings(html:&str,source:&Source,_base:&str)->Vec<Posting>{
 let anchor=Regex::new(r#"(?is)<a\b([^>]*)>(.*?)</a>"#).unwrap();let href_pattern=Regex::new(r#"(?is)\bhref\s*=\s*["']([^"']+)["']"#).unwrap();let label_pattern=Regex::new(r#"(?is)\baria-label\s*=\s*["']([^"']+)["']"#).unwrap();let path_pattern=Regex::new(r"(?:/about/careers/applications)?/jobs/results/([0-9]+)-").unwrap();let heading_pattern=Regex::new(r"(?is)<h3[^>]*>(.*?)</h3>").unwrap();let location_pattern=Regex::new(r"(?i)(?:Google|YouTube)\s*\|\s*(.+?)(?:\s+Minimum qualifications|\s+Preferred qualifications|\s+Early\s|\s+Mid\s|\s+Advanced\s|$)").unwrap();let mut postings=Vec::new();
 for captures in anchor.captures_iter(html){let Some(full_match)=captures.get(0)else{continue};let attributes=captures.get(1).map(|value|value.as_str()).unwrap_or_default();let Some(raw_url)=href_pattern.captures(attributes).and_then(|values|values.get(1)).map(|value|value.as_str())else{continue};let resolved=Url::parse(raw_url).ok().or_else(||Url::parse("https://www.google.com/about/careers/applications/").ok().and_then(|url|url.join(raw_url).ok()));let Some(mut resolved)=resolved else{continue};let host=resolved.host_str().unwrap_or_default();if host!="www.google.com"&&host!="careers.google.com"{continue;}let Some(external_id)=path_pattern.captures(resolved.path()).and_then(|values|values.get(1)).map(|value|value.as_str().to_string())else{continue};let card_start=html[..full_match.start()].rfind("<li").unwrap_or(full_match.start());let card_end=html[full_match.end()..].find("</li>").map(|offset|full_match.end()+offset+5).unwrap_or_else(||(full_match.end()+4000).min(html.len()));let card=&html[card_start..card_end];let label=label_pattern.captures(attributes).and_then(|values|values.get(1)).map(|value|value.as_str()).unwrap_or_default();let mut title=label.strip_prefix("Learn more about ").unwrap_or_default().trim().to_string();if title.is_empty(){title=heading_pattern.captures(card).and_then(|values|values.get(1)).map(|value|strip_html(value.as_str())).unwrap_or_default();}if title.is_empty(){continue;}let description=strip_html(card);let location=location_pattern.captures(&description).and_then(|values|values.get(1)).map(|value|value.as_str().trim().to_string()).unwrap_or_default();resolved.set_query(None);resolved.set_fragment(None);postings.push(Posting{external_id,title,company:source.company.clone(),location:location.clone(),remote_type:remote_type(&location,&description,""),posted_at:String::new(),apply_url:resolved.to_string(),description});}
 deduplicate(postings)
}

fn json_ld_postings(html:&str,source:&Source,base:&str)->Vec<Posting>{let expression=Regex::new(r#"(?is)<script[^>]*type\s*=\s*[\"']application/ld\+json[\"'][^>]*>(.*?)</script>"#).unwrap();let mut output=Vec::new();for captures in expression.captures_iter(html){if let Ok(value)=serde_json::from_str::<Value>(captures.get(1).map(|value|value.as_str()).unwrap_or_default()){collect_job_postings(&value,source,base,&mut output);}}deduplicate(output)}
fn embedded_job_postings(html:&str,source:&Source,base:&str)->Vec<Posting>{let expression=Regex::new(r#"(?is)<script[^>]*(?:type\s*=\s*[\"']application/json[\"']|id\s*=\s*[\"']__NEXT_DATA__[\"'])[^>]*>(.*?)</script>"#).unwrap();let mut output=Vec::new();for captures in expression.captures_iter(html){if let Ok(value)=serde_json::from_str::<Value>(captures.get(1).map(|value|value.as_str()).unwrap_or_default()){collect_job_like_objects(&value,source,base,&mut output);}}deduplicate(output)}
fn collect_job_postings(value:&Value,source:&Source,base:&str,output:&mut Vec<Posting>){match value{Value::Array(values)=>for value in values{collect_job_postings(value,source,base,output);},Value::Object(object)=>{let kind=object.get("@type").and_then(Value::as_str).unwrap_or_default();if kind.eq_ignore_ascii_case("JobPosting"){output.push(posting_from_object(object,source,base));}for nested in object.values(){collect_job_postings(nested,source,base,output);}},_=>{}}}
fn collect_job_like_objects(value:&Value,source:&Source,base:&str,output:&mut Vec<Posting>){match value{Value::Array(values)=>for value in values{collect_job_like_objects(value,source,base,output);},Value::Object(object)=>{let title=object.get("title").or_else(||object.get("name")).and_then(Value::as_str).unwrap_or_default();let id=object.get("id").or_else(||object.get("jobId")).or_else(||object.get("requisitionId"));let url=object.get("url").or_else(||object.get("applyUrl")).or_else(||object.get("jobUrl")).and_then(Value::as_str).unwrap_or_default();if !title.is_empty()&&id.is_some()&&(url.contains("job")||object.contains_key("description")){output.push(posting_from_object(object,source,base));}for nested in object.values(){collect_job_like_objects(nested,source,base,output);}},_=>{}}}

fn posting_from_object(object:&Map<String,Value>,source:&Source,base:&str)->Posting{let value=Value::Object(object.clone());let identifier=object.get("identifier").and_then(|value|value.get("value").or(Some(value))).and_then(Value::as_str).unwrap_or_default();let fallback=first_text(&value,&["id","jobId","requisitionId"]);let external_id=if identifier.is_empty(){fallback}else{identifier.to_string()};let description=strip_html(&first_text(&value,&["description","qualifications","summary"]));let location=structured_location(&value);let raw_url=first_text(&value,&["url","applyUrl","jobUrl"]);let apply_url=if raw_url.starts_with("http"){raw_url}else{Url::parse(base).ok().and_then(|url|url.join(&raw_url).ok()).map(|url|url.to_string()).unwrap_or_else(||base.to_string())};let organization=value.pointer("/hiringOrganization/name").and_then(Value::as_str).unwrap_or(&source.company).to_string();Posting{external_id:if external_id.is_empty(){apply_url.clone()}else{external_id},title:first_text(&value,&["title","name"]),company:organization,location:location.clone(),remote_type:remote_type(&location,&description,&first_text(&value,&["jobLocationType","workplaceType"])),posted_at:first_text(&value,&["datePosted","postedAt","updatedAt"]),apply_url,description}}
fn structured_location(value:&Value)->String{if let Some(location)=value.get("location").and_then(Value::as_str){return location.to_string();}let job_location=value.get("jobLocation").and_then(|location|location.as_array().and_then(|array|array.first()).or(Some(location))).unwrap_or(&Value::Null);let address=job_location.get("address").unwrap_or(job_location);["addressLocality","addressRegion","addressCountry"].iter().filter_map(|key|address.get(*key).and_then(Value::as_str)).filter(|value|!value.is_empty()).collect::<Vec<_>>().join(", ")}
fn deduplicate(values:Vec<Posting>)->Vec<Posting>{let mut seen=std::collections::HashSet::new();values.into_iter().filter(|posting|!posting.external_id.is_empty()&&seen.insert(posting.external_id.clone())).collect()}

#[cfg(test)]mod tests{use super::*;fn source(kind:&str)->Source{Source{id:"source".into(),kind:kind.into(),company:"Example".into(),board_token:"example".into(),location:None,enabled:true,last_refreshed_at:None,last_error:None,request_delay_ms:0,max_retries:0,max_results:0,page_size:0,max_calls:0,hydration_limit:0}}
 #[test]fn pagination_retains_the_first_final_response_url(){let mut value=None;retain_first_url(&mut value,"https://example.com/page/1".into());retain_first_url(&mut value,"https://example.com/page/2".into());assert_eq!(value.as_deref(),Some("https://example.com/page/1"));}
 #[test]fn pagination_continues_when_provider_caps_pages_below_requested_size(){assert!(needs_next_page(25,25,Some(120),5000,100));assert!(!needs_next_page(100,100,Some(120),100,100));}
 #[test]fn pagination_stops_on_empty_or_unreported_short_page(){assert!(!needs_next_page(100,0,Some(500),5000,100));assert!(!needs_next_page(25,25,None,5000,100));assert!(needs_next_page(100,100,None,5000,100));}
 #[test]fn amazon_uses_conservative_defaults(){assert_eq!(default_settings("Amazon"),ConnectorSettings{request_delay_ms:250,max_retries:4,max_results:1000,page_size:100,max_calls:10,hydration_limit:0});}
 #[test]fn redacts_connector_secrets_from_diagnostics(){let value=public_url("https://example.com/jobs?app_id=abc&app_key=secret&what=rust");assert!(value.contains("REDACTED"));assert!(!value.contains("secret"));}
 #[test]fn normalizes_amazon_feed(){let job=json!({"id":"42","title":"Software Engineer","location":"Toronto, ON, CAN","description_short":"Build APIs","job_path":"/en/jobs/42/software-engineer","posted_date":"2026-08-20"});let posting=normalize_amazon(&job,&source("Amazon"));assert_eq!(posting.external_id,"42");assert!(posting.apply_url.contains("amazon.jobs"));}
 #[test]fn extracts_jobposting_json_ld(){let html=r#"<script type="application/ld+json">{"@type":"JobPosting","identifier":{"value":"abc"},"title":"Engineering Manager","description":"<p>Lead teams</p>","datePosted":"2026-08-20","url":"https://example.com/jobs/abc","jobLocation":{"address":{"addressLocality":"Toronto","addressRegion":"ON","addressCountry":"Canada"}}}</script>"#;let jobs=json_ld_postings(html,&source("CareerPage"),"https://example.com/jobs");assert_eq!(jobs.len(),1);assert_eq!(jobs[0].location,"Toronto, ON, Canada");assert_eq!(jobs[0].description,"Lead teams");}
 #[test]fn extracts_google_careers_job_cards(){let html=r#"<div>3,471 jobs matched</div><ul><li><a href="/about/careers/applications/jobs/results/123-software-engineer"><h3>Software Engineer</h3></a><span>Google | Toronto, ON, Canada</span><h4>Minimum qualifications</h4><p>Build reliable systems.</p></li></ul>"#;let jobs=google_html_postings(html,&source("Google"),"https://www.google.com/about/careers/applications/jobs/results");assert_eq!(google_reported_count(html),Some(3471));assert_eq!(jobs.len(),1);assert_eq!(jobs[0].external_id,"123");assert_eq!(jobs[0].location,"Toronto, ON, Canada");assert!(jobs[0].apply_url.contains("123-software-engineer"));}
 #[test]fn extracts_current_google_relative_learn_more_links(){let html=r#"<main><div role="status">Showing 1 to 20 of 3,272 rows</div><ul><li><h3>Senior Software Engineer</h3><span>Google | Waterloo, ON, Canada</span><h4>Minimum qualifications</h4><p>Build systems.</p><a aria-label="Learn more about Senior Software Engineer" href="jobs/results/92823533872128710-senior-software-engineer/">Learn more</a></li></ul></main>"#;let jobs=google_html_postings(html,&source("Google"),"https://www.google.com/about/careers/applications/");assert_eq!(google_reported_count(html),Some(3272));assert_eq!(jobs.len(),1);assert_eq!(jobs[0].external_id,"92823533872128710");assert_eq!(jobs[0].title,"Senior Software Engineer");assert_eq!(jobs[0].location,"Waterloo, ON, Canada");}
 #[test]fn normalizes_microsoft_eightfold_positions(){let job=json!({"id":"1970393556928891","title":"Software Engineer","locations":["Toronto, ON, Canada"],"workLocationOption":"hybrid","postedTs":1787000000000i64,"positionUrl":"/careers/job/1970393556928891"});let posting=normalize_microsoft(&job,&source("Microsoft"));assert_eq!(posting.external_id,"1970393556928891");assert_eq!(posting.location,"Toronto, ON, Canada");assert_eq!(posting.remote_type,"Hybrid");assert!(posting.apply_url.contains("apply.careers.microsoft.com"));}
 #[test]fn unwraps_and_validates_current_microsoft_search_contract(){let envelope=json!({"status":200,"data":{"count":1,"positions":[{"id":1970393556928891i64,"name":"Software Engineer","locations":["Canada, Ontario, Toronto"],"positionUrl":"/careers/job/1970393556928891"}]}});let source=source("Microsoft");let payload=microsoft_payload(&envelope,&source,"search").unwrap();assert_eq!(payload.get("count").and_then(Value::as_i64),Some(1));validate_microsoft_summary(&payload["positions"][0],&source).unwrap();}
 #[test]fn microsoft_fingerprint_changes_only_when_summary_contract_fields_change(){let original=json!({"id":42,"name":"Engineer","locations":["Toronto"],"postedTs":100,"creationTs":90,"workLocationOption":"hybrid","positionUrl":"/careers/job/42","solrScore":1});let irrelevant=json!({"id":42,"name":"Engineer","locations":["Toronto"],"postedTs":100,"creationTs":90,"workLocationOption":"hybrid","positionUrl":"/careers/job/42","solrScore":99});let changed=json!({"id":42,"name":"Senior Engineer","locations":["Toronto"],"postedTs":100,"creationTs":90,"workLocationOption":"hybrid","positionUrl":"/careers/job/42"});assert_eq!(microsoft_summary_fingerprint(&original),microsoft_summary_fingerprint(&irrelevant));assert_ne!(microsoft_summary_fingerprint(&original),microsoft_summary_fingerprint(&changed));}
 #[test]fn microsoft_detail_overrides_summary_and_supplies_full_description(){let summary=json!({"id":42,"name":"Engineer","locations":["Toronto"],"positionUrl":"/careers/job/42"});let detail=json!({"id":42,"name":"Senior Engineer","jobDescription":"<p>Build reliable systems.</p>","publicUrl":"https://apply.careers.microsoft.com/careers/job/42"});let posting=normalize_microsoft(&merge_microsoft_detail(&summary,&detail),&source("Microsoft"));assert_eq!(posting.title,"Senior Engineer");assert_eq!(posting.description,"Build reliable systems.");}
 #[test]fn classifies_known_provider_urls(){assert_eq!(provider_from_url("https://jobs.ashbyhq.com/Example"),Some("Ashby"));assert_eq!(provider_from_url("https://example.wd5.myworkdayjobs.com/Careers"),Some("Workday"));}}
