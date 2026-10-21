use chrono::{Datelike, Utc};
use regex::Regex;
use crate::{geocoding, models::{Posting, Profile, ProfileMatch, ResolvedLocation, ScoreComponent}};

pub const SCORING_VERSION:&str="2.1";

#[derive(Debug, Clone)]
pub struct LocationEvaluation { pub eligible:bool,pub status:String,pub distance_km:Option<f64>,pub details:String }

const SKILL_ALIASES:&[(&str,&[&str])]=&[
 ("aws",&["amazon web services"]),("azure",&["microsoft azure"]),("gcp",&["google cloud","google cloud platform"]),
 ("javascript",&["js"]),("typescript",&["ts"]),("node.js",&["nodejs","node js"]),("react",&["react.js","reactjs"]),
 ("postgresql",&["postgres","postgre sql"]),("sql server",&["mssql","microsoft sql server"]),("kubernetes",&["k8s"]),
 ("continuous integration",&["ci/cd","ci cd","continuous delivery"]),("leadership",&["people management","team leadership","engineering leadership"]),
 ("project management",&["program management"]),("machine learning",&["ml"]),("artificial intelligence",&["ai"]),
 ("rest api",&["restful api","rest apis"]),("dotnet",&[".net","asp.net"]),("amazon ecs",&["ecs"]),
];

fn terms(values: &[String]) -> Vec<String> { values.iter().map(|v| v.trim().to_lowercase()).filter(|v| !v.is_empty()).collect() }
fn ratio(count: usize,total: usize)->f64 { if total==0 {0.0} else {(count as f64/total as f64).min(1.0)} }

fn normalized(value:&str)->String{format!(" {} ",value.to_lowercase().chars().map(|c|if c.is_alphanumeric(){c}else{' '}).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" "))}
fn aliases(value:&str)->Vec<String>{let needle=normalized(value);for(canonical,values)in SKILL_ALIASES{if normalized(canonical)==needle||values.iter().any(|v|normalized(v)==needle){return std::iter::once(*canonical).chain(values.iter().copied()).map(str::to_string).collect();}}vec![value.to_string()]}
fn contains_term(text:&str,value:&str)->bool{aliases(value).iter().any(|candidate|text.contains(&normalized(candidate)))}
fn matching_terms(text:&str,values:&[String])->Vec<String>{values.iter().filter(|v|!v.trim().is_empty()&&contains_term(text,v)).cloned().collect()}
fn unique(mut values:Vec<String>)->Vec<String>{values.sort_by_key(|v|v.to_lowercase());values.dedup_by(|a,b|a.eq_ignore_ascii_case(b));values}

#[derive(Debug)] struct ResumeAnalysis{years:Option<f64>}
#[derive(Debug)] struct JobAnalysis{required_years:Option<f64>,education_terms:Vec<String>,detected_terms:Vec<String>}

fn years_in_resume(text:&str)->Option<f64>{
 let explicit=Regex::new(r"(?i)(\d{1,2})(?:\+)?\s+years?").unwrap().captures_iter(text).filter_map(|c|c[1].parse::<f64>().ok()).fold(0.0_f64,f64::max);
 let current=Utc::now().year();let ranges=Regex::new(r"(?i)((?:19|20)\d{2})\s*(?:-|–|—|to)\s*(present|current|(?:19|20)\d{2})").unwrap().captures_iter(text).filter_map(|c|{let start=c[1].parse::<i32>().ok()?;let end=match c[2].to_lowercase().as_str(){"present"|"current"=>current,v=>v.parse().ok()?};Some((end-start).clamp(0,40)as f64)}).sum::<f64>().min(40.0);
 let value=explicit.max(ranges);if value>0.0{Some(value)}else{None}
}
fn required_years(text:&str)->Option<f64>{Regex::new(r"(?i)(\d{1,2})(?:\+)?\s+(?:or more\s+)?years?(?:\s+of)?").unwrap().captures_iter(text).filter_map(|c|c[1].parse::<f64>().ok()).max_by(|a,b|a.total_cmp(b))}
fn analyze_resume(text:&str)->ResumeAnalysis{ResumeAnalysis{years:years_in_resume(text)}}
fn analyze_job(posting:&Posting,profile:&Profile)->JobAnalysis{
 let corpus=normalized(&format!("{} {}",posting.title,posting.description));
 let mut candidates=profile.required_skills.iter().chain(profile.preferred_skills.iter()).cloned().collect::<Vec<_>>();candidates.extend(SKILL_ALIASES.iter().map(|(v,_)|v.to_string()));
 let detected_terms=unique(candidates.into_iter().filter(|v|contains_term(&corpus,v)).collect());
 let education_terms=["bachelor degree","bachelor's degree","masters degree","master's degree","phd","doctorate","certification"].into_iter().filter(|v|contains_term(&corpus,v)).map(str::to_string).collect();
 JobAnalysis{required_years:required_years(&posting.description),education_terms,detected_terms}
}

const CANADA_MARKERS: &[&str] = &[
    "canada", "alberta", "ab", "british columbia", "bc", "manitoba", "mb",
    "new brunswick", "nb", "newfoundland and labrador", "nl", "nova scotia", "ns",
    "northwest territories", "nt", "nunavut", "nu", "ontario", "on",
    "prince edward island", "pe", "quebec", "qc", "saskatchewan", "sk", "yukon", "yt",
    "toronto", "ottawa", "kitchener", "waterloo", "vancouver", "montreal", "calgary",
    "edmonton", "winnipeg", "halifax", "saskatoon", "regina", "mississauga", "brampton",
    "markham", "burnaby", "surrey",
];

const UNITED_STATES_MARKERS: &[&str] = &[
    "united states", "united states of america", "usa", "us", "alabama", "al", "alaska", "ak",
    "arizona", "az", "arkansas", "ar", "california", "ca", "colorado", "co", "connecticut", "ct",
    "delaware", "de", "district of columbia", "dc", "florida", "fl", "georgia", "ga", "hawaii", "hi",
    "idaho", "id", "illinois", "il", "indiana", "in", "iowa", "ia", "kansas", "ks", "kentucky", "ky",
    "louisiana", "la", "maine", "me", "maryland", "md", "massachusetts", "ma", "michigan", "mi",
    "minnesota", "mn", "mississippi", "ms", "missouri", "mo", "montana", "mt", "nebraska", "ne",
    "nevada", "nv", "new hampshire", "nh", "new jersey", "nj", "new mexico", "nm", "new york", "ny",
    "north carolina", "nc", "north dakota", "nd", "ohio", "oh", "oklahoma", "ok", "oregon", "or",
    "pennsylvania", "pa", "rhode island", "ri", "south carolina", "sc", "south dakota", "sd",
    "tennessee", "tn", "texas", "tx", "utah", "ut", "vermont", "vt", "virginia", "va",
    "washington", "wa", "west virginia", "wv", "wisconsin", "wi", "wyoming", "wy",
];

fn normalized_location(value: &str) -> String {
    let words = value.to_lowercase().chars().map(|character| if character.is_alphanumeric() { character } else { ' ' }).collect::<String>();
    format!(" {} ", words.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn contains_location_term(location: &str, value: &str) -> bool {
    let term = normalized_location(value);
    term.trim().is_empty() || location.contains(&term)
}

fn country_matches(location: &str, country: &str) -> bool {
    let normalized_country = normalized_location(country);
    let country_name = normalized_country.trim();
    if country_name.is_empty() { return false; }
    if contains_location_term(location, country_name) { return true; }
    let markers = match country_name {
        "canada" | "canadian" => CANADA_MARKERS,
        "united states" | "united states of america" | "usa" | "us" | "america" => UNITED_STATES_MARKERS,
        _ => return false,
    };
    markers.iter().any(|marker| contains_location_term(location, marker))
}

fn location_group_matches(location: &str, values: &[String], countries: bool) -> bool {
    let configured = terms(values);
    configured.is_empty() || configured.iter().any(|value| if countries { country_matches(location, value) } else { contains_location_term(location, value) })
}

pub fn location_matches(posting: &Posting, profile: &Profile) -> bool {
    if profile.distance_enabled { return evaluate_location(posting,profile,&[]).eligible; }
    let location=normalized_location(&posting.location);
    if profile.remote_only { return posting.remote_type == "Remote" || location.contains("remote"); }
    location_group_matches(&location, &profile.countries, true)
        && location_group_matches(&location, &profile.regions, false)
        && location_group_matches(&location, &profile.cities, false)
}

pub fn distance_km(latitude_a:f64,longitude_a:f64,latitude_b:f64,longitude_b:f64)->f64{
    let earth_radius=6371.0_f64;let lat_a=latitude_a.to_radians();let lat_b=latitude_b.to_radians();let delta_lat=(latitude_b-latitude_a).to_radians();let delta_lon=(longitude_b-longitude_a).to_radians();let value=((delta_lat/2.0).sin().powi(2)+lat_a.cos()*lat_b.cos()*(delta_lon/2.0).sin().powi(2)).clamp(0.0,1.0);2.0*earth_radius*value.sqrt().asin()
}

pub fn evaluate_location(posting:&Posting,profile:&Profile,resolved:&[ResolvedLocation])->LocationEvaluation{
    if !profile.distance_enabled{return LocationEvaluation{eligible:location_matches_legacy(posting,profile),status:"Legacy location filters".into(),distance_km:None,details:"Country, province/state, and city filters applied.".into()};}
    let normalized=normalized_location(&posting.location);
    if posting.remote_type=="Remote"||normalized.contains(" remote "){
        return LocationEvaluation{eligible:profile.include_remote,status:"Remote eligible".into(),distance_km:None,details:"Remote job; commuting distance is not applicable. Review any employer jurisdiction restrictions shown in the posting.".into()};
    }
    let style_allowed=match posting.remote_type.as_str(){"Hybrid"=>profile.include_hybrid,"On-site"=>profile.include_onsite,_=>profile.include_hybrid||profile.include_onsite};
    if !style_allowed{return LocationEvaluation{eligible:false,status:"Work style excluded".into(),distance_km:None,details:format!("{} jobs are disabled for this profile.",posting.remote_type)};}
    let (Some(home_lat),Some(home_lon))=(profile.active_latitude,profile.active_longitude)else{return LocationEvaluation{eligible:false,status:"Profile location unresolved".into(),distance_km:None,details:"Save a resolvable active profile location before using distance filtering.".into()};};
    let nearest=resolved.iter().map(|candidate|(distance_km(home_lat,home_lon,candidate.latitude,candidate.longitude),candidate)).min_by(|left,right|left.0.total_cmp(&right.0));
    if let Some((distance,candidate))=nearest{let eligible=distance<=profile.maximum_distance_km;return LocationEvaluation{eligible,status:if eligible{"Within range".into()}else{"Outside range".into()},distance_km:Some(distance),details:format!("{distance:.1} km from {} to {}; maximum {:.1} km.",profile.active_location,candidate.label,profile.maximum_distance_km)};}
    let broad=geocoding::is_broad_location(&posting.location);let review=profile.broad_location_policy=="review";
    LocationEvaluation{eligible:review,status:if broad{"Broad location".into()}else{"Unresolved location".into()},distance_km:None,details:if broad{"The posting provides only a province/state or country. Distance was not estimated from a geographic centre.".into()}else{"The job location could not be resolved to reliable coordinates.".into()}}
}

fn location_matches_legacy(posting:&Posting,profile:&Profile)->bool{
    let location=normalized_location(&posting.location);if profile.remote_only{return posting.remote_type=="Remote"||location.contains("remote");}location_group_matches(&location,&profile.countries,true)&&location_group_matches(&location,&profile.regions,false)&&location_group_matches(&location,&profile.cities,false)
}

pub fn score(posting:&Posting, profile:&Profile)->ProfileMatch { let location=evaluate_location(posting,profile,&[]);score_with_location(posting,profile,&location) }

pub fn score_with_location(posting:&Posting, profile:&Profile, location:&LocationEvaluation)->ProfileMatch {
    let corpus=normalized(&format!("{} {}",posting.title,posting.description));let title=normalized(&posting.title);
    let resume=analyze_resume(&profile.resume_text);let job=analyze_job(posting,profile);let w=&profile.weights;
    let title_hits=matching_terms(&title,&profile.titles);let required=matching_terms(&corpus,&profile.required_skills);let preferred=matching_terms(&corpus,&profile.preferred_skills);
    let missing_required=profile.required_skills.iter().filter(|v|!required.iter().any(|m|m.eq_ignore_ascii_case(v))).cloned().collect::<Vec<_>>();
    let resume_corpus=normalized(&profile.resume_text);let resume_matches=job.detected_terms.iter().filter(|v|contains_term(&resume_corpus,v)).cloned().collect::<Vec<_>>();
    let mut components=Vec::new();
    if !profile.titles.is_empty(){let factor=if title_hits.is_empty(){let wanted=profile.titles.iter().flat_map(|v|normalized(v).split_whitespace().map(str::to_string).collect::<Vec<_>>()).collect::<Vec<_>>();let job_words=title.split_whitespace().collect::<Vec<_>>();ratio(wanted.iter().filter(|v|job_words.contains(&v.as_str())).count(),wanted.len())*0.6}else{1.0};components.push(ScoreComponent{key:"title".into(),label:"Title and seniority".into(),earned:factor*w.title as f64,possible:w.title,details:if title_hits.is_empty(){"Partial title/seniority overlap".into()}else{format!("Matched {}",title_hits.join(", "))}});}
    if !profile.required_skills.is_empty(){components.push(ScoreComponent{key:"requiredSkills".into(),label:"Required skill coverage".into(),earned:ratio(required.len(),profile.required_skills.len())*w.required_skills as f64,possible:w.required_skills,details:format!("{} of {} configured skills found",required.len(),profile.required_skills.len())});}
    if !profile.preferred_skills.is_empty(){components.push(ScoreComponent{key:"preferredSkills".into(),label:"Preferred skill coverage".into(),earned:ratio(preferred.len(),profile.preferred_skills.len())*w.preferred_skills as f64,possible:w.preferred_skills,details:format!("{} of {} preferred skills found",preferred.len(),profile.preferred_skills.len())});}
    if !profile.resume_text.trim().is_empty()&&!job.detected_terms.is_empty(){components.push(ScoreComponent{key:"resumeEvidence".into(),label:"Resume evidence".into(),earned:ratio(resume_matches.len(),job.detected_terms.len())*w.resume_evidence as f64,possible:w.resume_evidence,details:format!("Resume supports {} of {} detected job skills",resume_matches.len(),job.detected_terms.len())});}
    if let Some(required_years)=job.required_years{let factor=resume.years.map(|v|(v/required_years).min(1.0)).unwrap_or(0.0);components.push(ScoreComponent{key:"experience".into(),label:"Experience alignment".into(),earned:factor*w.experience as f64,possible:w.experience,details:match resume.years{Some(v)=>format!("Estimated {v:.1} resume years vs {required_years:.1} required"),None=>format!("Job asks for {required_years:.1} years; resume years were not detected")}});}
    if !job.education_terms.is_empty(){let matched=job.education_terms.iter().filter(|v|contains_term(&resume_corpus,v)).count();components.push(ScoreComponent{key:"education".into(),label:"Education and certifications".into(),earned:ratio(matched,job.education_terms.len())*w.education as f64,possible:w.education,details:format!("{matched} of {} education/certification requirements found",job.education_terms.len())});}
    if profile.distance_enabled&&posting.remote_type!="Remote"{if let Some(distance)=location.distance_km{let factor=(1.0-distance/profile.maximum_distance_km.max(1.0)).clamp(0.0,1.0);components.push(ScoreComponent{key:"proximity".into(),label:"Commute proximity".into(),earned:factor*w.proximity as f64,possible:w.proximity,details:location.details.clone()});}}
    let possible=components.iter().map(|v|v.possible.max(0)).sum::<i32>().max(1)as f64;let earned=components.iter().map(|v|v.earned.max(0.0)).sum::<f64>();let match_score=((earned/possible)*100.0).min(100.0).round()as i32;
    let matched_terms=unique(required.iter().chain(preferred.iter()).chain(resume_matches.iter()).cloned().collect());
    let missing_note=if missing_required.is_empty(){String::new()}else{format!(" Missing configured skills: {}.",missing_required.join(", "))};
    let match_reason=format!("{}% fit: {}.{} Location: {} — {}",match_score,components.iter().map(|v|format!("{} {:.0}/{:.0}",v.label,v.earned,v.possible)).collect::<Vec<_>>().join("; "),missing_note,location.status,location.details);
    ProfileMatch{profile_id:profile.id.clone(),profile_name:profile.name.clone(),match_score,match_reason,score_breakdown:components,matched_terms,missing_terms:missing_required,resume_years:resume.years,required_years:job.required_years,algorithm_version:SCORING_VERSION.into(),distance_km:location.distance_km,location_status:location.status.clone(),location_details:location.details.clone()}
}

#[cfg(test)] mod tests {
 use super::*; use crate::models::Weights;
 fn profile()->Profile{Profile{id:"p".into(),name:"Backend".into(),titles:vec!["engineer".into()],required_skills:vec!["rust".into()],preferred_skills:vec![],remote_only:true,countries:vec![],regions:vec![],cities:vec![],min_score:30,enabled:true,schedule:"Manual".into(),resume_text:"".into(),weights:Weights::default(),distance_enabled:false,active_location:"".into(),active_latitude:None,active_longitude:None,maximum_distance_km:75.0,include_remote:true,include_hybrid:true,include_onsite:true,broad_location_policy:"review".into()}}
 fn posting(location:&str)->Posting{Posting{external_id:"1".into(),title:"Rust Engineer".into(),company:"A".into(),location:location.into(),remote_type:"On-site".into(),posted_at:Utc::now().to_rfc3339(),apply_url:"x".into(),description:"Rust".into()}}
 #[test] fn scores_and_filters_remote(){let p=Posting{external_id:"1".into(),title:"Rust Engineer".into(),company:"A".into(),location:"Remote".into(),remote_type:"Remote".into(),posted_at:Utc::now().to_rfc3339(),apply_url:"x".into(),description:"Rust".into()};assert!(location_matches(&p,&profile()));assert!(score(&p,&profile()).match_score>=70);}
 #[test] fn country_only_matches_canadian_province_and_city_locations(){let mut p=profile();p.remote_only=false;p.countries=vec!["Canada".into()];assert!(location_matches(&posting("Toronto, ON"),&p));assert!(location_matches(&posting("Kitchener, Ontario"),&p));assert!(!location_matches(&posting("Austin, TX"),&p));}
 #[test] fn country_only_matches_us_state_locations(){let mut p=profile();p.remote_only=false;p.countries=vec!["United States".into()];assert!(location_matches(&posting("Boston, MA"),&p));assert!(location_matches(&posting("New York, New York"),&p));assert!(!location_matches(&posting("Toronto, ON"),&p));}
 #[test] fn empty_region_and_city_lists_do_not_filter(){let mut p=profile();p.remote_only=false;p.countries=vec!["Canada".into()];p.regions=vec![];p.cities=vec![];assert!(location_matches(&posting("Vancouver, BC"),&p));}
 #[test] fn aliases_match_complete_phrases(){let mut p=profile();p.required_skills=vec!["PostgreSQL".into(),"AWS".into()];let mut job=posting("Remote");job.remote_type="Remote".into();job.description="Operate Postgres on Amazon Web Services.".into();let result=score(&job,&p);assert!(result.matched_terms.contains(&"PostgreSQL".to_string()));assert!(result.matched_terms.contains(&"AWS".to_string()));assert_eq!(result.algorithm_version,SCORING_VERSION);}
 #[test] fn resume_and_experience_evidence_are_explained(){let mut p=profile();p.required_skills=vec!["PostgreSQL".into()];p.resume_text="Software Engineer, 2016 - Present. Built PostgreSQL services and led teams.".into();let mut job=posting("Remote");job.remote_type="Remote".into();job.description="Requires 5+ years of experience with Postgres and leadership.".into();let result=score(&job,&p);assert!(result.resume_years.unwrap_or_default()>=9.0);assert_eq!(result.required_years,Some(5.0));assert!(result.score_breakdown.iter().any(|v|v.key=="resumeEvidence"&&v.earned>0.0));assert!(result.score_breakdown.iter().any(|v|v.key=="experience"&&v.earned==v.possible as f64));}
 #[test] fn absent_categories_do_not_reduce_the_score(){let mut p=profile();p.titles.clear();p.preferred_skills.clear();p.resume_text.clear();let mut job=posting("Remote");job.remote_type="Remote".into();assert_eq!(score(&job,&p).match_score,100);}
 #[test] fn insufficient_experience_reduces_the_score(){let mut p=profile();p.resume_text="Software Engineer, 2024 - Present. Rust.".into();let mut job=posting("Remote");job.remote_type="Remote".into();job.description="Rust role requiring 10+ years of experience.".into();let result=score(&job,&p);let experience=result.score_breakdown.iter().find(|v|v.key=="experience").unwrap();assert!(experience.earned<experience.possible as f64);assert!(result.match_score<100);}
 #[test] fn distance_range_includes_nearby_and_rejects_distant_jobs(){let mut p=profile();p.distance_enabled=true;p.remote_only=false;p.active_location="Kitchener, Ontario".into();p.active_latitude=Some(43.4516);p.active_longitude=Some(-80.4925);p.maximum_distance_km=75.0;let mut job=posting("Toronto, ON");job.remote_type="Hybrid".into();let near=ResolvedLocation{label:"Waterloo, Ontario".into(),latitude:43.4643,longitude:-80.5204,confidence:"city".into()};let far=ResolvedLocation{label:"Toronto, Ontario".into(),latitude:43.6532,longitude:-79.3832,confidence:"city".into()};assert!(evaluate_location(&job,&p,&[near]).eligible);assert!(!evaluate_location(&job,&p,&[far]).eligible);}
 #[test] fn remote_jobs_bypass_distance_and_broad_jobs_are_reviewable(){let mut p=profile();p.distance_enabled=true;p.active_location="Kitchener, Ontario".into();p.active_latitude=Some(43.4516);p.active_longitude=Some(-80.4925);let mut job=posting("Remote");job.remote_type="Remote".into();assert!(evaluate_location(&job,&p,&[]).eligible);job.remote_type="Hybrid".into();job.location="Ontario, Canada".into();let review=evaluate_location(&job,&p,&[]);assert!(review.eligible);assert_eq!(review.status,"Broad location");p.broad_location_policy="exclude".into();assert!(!evaluate_location(&job,&p,&[]).eligible);}
 #[test] fn proximity_weight_is_omitted_for_remote_jobs(){let mut p=profile();p.distance_enabled=true;p.active_location="Kitchener".into();p.active_latitude=Some(43.4516);p.active_longitude=Some(-80.4925);let mut job=posting("Remote");job.remote_type="Remote".into();let evaluation=evaluate_location(&job,&p,&[]);assert!(!score_with_location(&job,&p,&evaluation).score_breakdown.iter().any(|value|value.key=="proximity"));}
 #[test] fn proximity_weight_rewards_nearer_eligible_jobs(){let mut p=profile();p.distance_enabled=true;p.remote_only=false;p.active_location="Kitchener".into();p.active_latitude=Some(43.4516);p.active_longitude=Some(-80.4925);p.maximum_distance_km=75.0;let mut job=posting("Waterloo, ON");job.remote_type="Hybrid".into();let office=ResolvedLocation{label:"Waterloo, Ontario".into(),latitude:43.4643,longitude:-80.5204,confidence:"city".into()};let evaluation=evaluate_location(&job,&p,&[office]);let result=score_with_location(&job,&p,&evaluation);let proximity=result.score_breakdown.iter().find(|value|value.key=="proximity").unwrap();assert!(proximity.earned>0.0);assert!(proximity.earned<=proximity.possible as f64);}
}
