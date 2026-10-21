use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;
use web_push::{ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder};
use crate::{error::ApiError, AppState};

pub async fn create(state:&AppState,user:Uuid,title:&str,body:&str,url:&str)->Result<(),ApiError>{
 sqlx::query("INSERT INTO notifications(user_id,title,body,url) VALUES($1,$2,$3,$4)").bind(user).bind(title).bind(body).bind(url).execute(&state.db).await?;
 let Some(private_key)=state.config.vapid_private_key.as_deref() else{return Ok(())};
 let rows=sqlx::query_as::<_,(String,String,String)>("SELECT endpoint,p256dh,auth FROM push_subscriptions WHERE user_id=$1").bind(user).fetch_all(&state.db).await?;
 let client=IsahcWebPushClient::new().map_err(|e|ApiError::Internal(format!("push client: {e}")))?;
 let payload=serde_json::to_vec(&serde_json::json!({"title":title,"body":body,"url":url}))?;
 for(endpoint,p256dh,auth) in rows { let subscription=SubscriptionInfo::new(endpoint,p256dh,auth);let mut signature=match VapidSignatureBuilder::from_base64(private_key,&subscription){Ok(v)=>v,Err(e)=>{tracing::warn!(%e,"invalid VAPID key");continue}};signature.add_claim("sub",state.config.vapid_subject.clone());let mut builder=WebPushMessageBuilder::new(&subscription);builder.set_payload(ContentEncoding::Aes128Gcm,&payload);match signature.build(){Ok(v)=>builder.set_vapid_signature(v),Err(e)=>{tracing::warn!(%e,"VAPID signature failed");continue}};if let Err(e)=client.send(builder.build().map_err(|e|ApiError::Internal(e.to_string()))?).await{tracing::warn!(%e,"push delivery failed");} }
 Ok(())
}

pub fn public_key_bytes(key:&str)->Result<Vec<u8>,ApiError>{URL_SAFE_NO_PAD.decode(key).map_err(|_|ApiError::BadRequest("invalid VAPID public key".into()))}
