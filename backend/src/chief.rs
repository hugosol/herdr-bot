//! Backend-wide chief authority. Reservations precede external effects; native
//! identities come only from trusted launch/resume provenance, never Task names.
//! Registered membership is transient. Pending/uncertain external effects and
//! revoked identities survive restart independently of process liveness.
use std::{collections::BTreeMap, io, path::PathBuf, sync::Mutex};
use axum::{Json, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::{AppState, files::FileStore};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Claim {
    pub claim_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub phase: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ledger {
    current: Option<Claim>,
    /// True permits re-registration after backend restart, false requires an
    /// explicit user Resume. Old processes cannot undo expiry or unregister.
    known: BTreeMap<String,bool>,
    /// Explicit release revokes authority, not the observed external outcome.
    retired: Vec<Claim>,
}

pub(crate) struct Authority {
    file: PathBuf,
    files: FileStore,
    initialized: Mutex<bool>,
    /// Only membership transitions hold this permit across a session-table await.
    /// External Herdr calls never hold it.
    pub membership: tokio::sync::Mutex<()>,
}
impl Authority {
    pub fn new(file: PathBuf) -> Self {
        Self { file, files: FileStore, initialized:Mutex::new(false), membership:tokio::sync::Mutex::new(()) }
    }
    fn with<T>(&self, operation: impl FnOnce(&mut Ledger) -> Result<T, Error>) -> Result<T, Error> {
        let mut initialized = self.initialized.lock().unwrap_or_else(|error|error.into_inner());
        self.files.with(std::slice::from_ref(&self.file), |access| {
            let mut ledger: Ledger = match access.read(&self.file) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ledger::default(),
                Err(error) => return Err(error.into()),
            };
            let before = serde_json::to_vec(&ledger).map_err(io::Error::other)?;
            if !*initialized && ledger.current.as_ref().is_some_and(|claim|claim.phase=="registered") {
                // ADR-0006: backend restart clears registrations. Provenance and
                // revocation remain, so concurrent callers must acquire anew.
                ledger.current = None;
            }
            let result = operation(&mut ledger);
            let after = serde_json::to_vec(&ledger).map_err(io::Error::other)?;
            if before != after {
                if let Some(parent) = self.file.parent() { access.create_dir_all(parent)?; }
                access.replace(&self.file,&after)?;
            }
            *initialized = true;
            result
        }).map_err(Error::from)?
    }
    pub fn current(&self) -> Result<Option<Claim>,Error> { self.with(|ledger|Ok(ledger.current.clone())) }
    pub fn reserve_creation(&self, creation_id: &str) -> Result<(),Error> {
        self.with(|ledger| {
            if ledger.current.is_some() { return Err(Error::Occupied); }
            ledger.known.values_mut().for_each(|eligible|*eligible=false);
            ledger.current = Some(Claim { claim_id: crate::chats::new_creation_id(), creation_id: Some(creation_id.into()),session_id:None,phase:"starting".into() });
            Ok(())
        })
    }
    pub fn creation_failed(&self, creation_id: &str, definite: bool) -> Result<(),Error> {
        self.with(|ledger| {
            let Some(claim) = ledger.current.as_mut().filter(|claim|claim.creation_id.as_deref()==Some(creation_id)) else { return Ok(()); };
            if definite && claim.session_id.is_none() { ledger.current = None; }
            else if claim.phase != "registered" { claim.phase = "uncertain".into(); }
            Ok(())
        })
    }
    pub fn bind(&self, creation_id: &str, session_id: &str, registered: bool) -> Result<(),Error> {
        self.with(|ledger| {
            let Some(claim) = ledger.current.as_mut().filter(|claim|claim.creation_id.as_deref()==Some(creation_id)) else { return Err(Error::NotCurrent); };
            if claim.session_id.as_deref().is_some_and(|id|id!=session_id) { return Err(Error::NotCurrent); }
            claim.session_id = Some(session_id.into());
            claim.phase = if registered { "registered" } else { "awaiting-registration" }.into();
            ledger.known.insert(session_id.into(),true);
            Ok(())
        })
    }
    pub fn reserve_resume(&self, session_id: &str) -> Result<String,Error> {
        self.with(|ledger| {
            if let Some(current) = &ledger.current {
                if current.session_id.as_deref()==Some(session_id) && current.phase=="registered" { return Ok(current.claim_id.clone()); }
                return Err(Error::Occupied);
            }
            let claim_id = crate::chats::new_creation_id();
            ledger.known.values_mut().for_each(|eligible|*eligible=false);
            ledger.current = Some(Claim { claim_id:claim_id.clone(), creation_id:None,session_id:Some(session_id.into()),phase:"resuming".into() });
            ledger.known.insert(session_id.into(),true);
            Ok(claim_id)
        })
    }
    pub fn is_known(&self, session_id: &str) -> Result<bool,Error> { self.with(|ledger|Ok(ledger.known.contains_key(session_id))) }
    pub fn restoring_effect(&self, session_id: &str, definite_failure: bool) -> Result<(),Error> {
        self.with(|ledger| {
            if !ledger.known.contains_key(session_id) { return Ok(()); }
            let Some(claim) = ledger.current.as_mut().filter(|claim|claim.session_id.as_deref()==Some(session_id)) else { return Err(Error::NotCurrent); };
            if claim.phase!="registered" {
                if definite_failure { ledger.current=None; ledger.known.insert(session_id.into(),false); }
                else { claim.phase="uncertain".into(); }
            }
            Ok(())
        })
    }
    pub fn resume_finished(&self, claim_id: &str, success: bool) -> Result<(),Error> {
        self.with(|ledger| {
            let Some(claim) = ledger.current.as_mut().filter(|claim|claim.claim_id==claim_id) else { return if success { Err(Error::NotCurrent) } else { Ok(()) }; };
            if success && claim.phase!="registered" { claim.phase="awaiting-registration".into(); }
            else if !success && claim.phase=="resuming" { ledger.current=None; }
            Ok(())
        })
    }
    pub fn register(&self, session_id: &str) -> Result<(),Error> {
        self.with(|ledger| {
            let Some(eligible) = ledger.known.get(session_id).copied() else { return Ok(()); };
            match &mut ledger.current {
                Some(claim) if claim.session_id.as_deref()==Some(session_id) => { claim.phase="registered".into(); }
                Some(_) => return Err(Error::Occupied),
                None if eligible => {
                    ledger.known.values_mut().for_each(|eligible|*eligible=false);
                    ledger.known.insert(session_id.into(),true);
                    ledger.current = Some(Claim { claim_id:crate::chats::new_creation_id(),creation_id:None,session_id:Some(session_id.into()),phase:"registered".into() });
                }
                None => return Err(Error::NotCurrent),
            }
            Ok(())
        })
    }
    pub fn remove(&self, session_id: &str) -> Result<(),Error> {
        self.with(|ledger| {
            if ledger.known.contains_key(session_id) { ledger.known.insert(session_id.into(),false); }
            if ledger.current.as_ref().is_some_and(|claim|claim.session_id.as_deref()==Some(session_id)) { ledger.current=None; }
            Ok(())
        })
    }
    pub fn release(&self, claim_id:&str) -> Result<Claim,Error> {
        self.with(|ledger| {
            if !ledger.current.as_ref().is_some_and(|claim|claim.claim_id==claim_id) { return Err(Error::NotCurrent); }
            let claim=ledger.current.take().expect("matched current claim");
            if let Some(session_id)=&claim.session_id { ledger.known.insert(session_id.clone(),false); }
            ledger.retired.push(claim.clone());
            Ok(claim)
        })
    }
    /// Consumers must call this at the actual dispatch/receipt authorization
    /// point, not cache a previous success or infer authority from Task type.
    pub fn qualify(&self, session_id: &str) -> Result<Claim,Error> {
        self.with(|ledger|ledger.current.as_ref().filter(|claim|claim.session_id.as_deref()==Some(session_id) && claim.phase=="registered").cloned().ok_or(Error::NotCurrent))
    }
}
pub(crate) async fn is_chief(state:&AppState,selected:&crate::chats::history::HistorySelection) -> Result<bool,Response> {
    if state.chief.is_known(&selected.session_id).map_err(Error::response)? { return Ok(true); }
    let Some(source) = &selected.source_task_id else { return Ok(false); };
    match state.tasks.get(source).await {
        Ok(task) => Ok(task.task_type==crate::tasks::TaskType::Chief),
        Err(crate::tasks::TaskError::NotFound) => Ok(false),
        Err(error) => Err(crate::chats::resume::settings_failure(selected.agent_type.as_deref(),&error)),
    }
}

#[derive(Debug)]
pub(crate) enum Error { Occupied, NotCurrent, Storage(io::Error) }
impl From<io::Error> for Error { fn from(error: io::Error) -> Self { Self::Storage(error) } }
impl Error {
    pub fn response(self) -> Response {
        match self {
            Self::Occupied => crate::rejected_owned(StatusCode::CONFLICT,"chief-occupied","当前 chief 或启动占位尚未移除；本次未发起。".into()),
            Self::NotCurrent => crate::rejected_owned(StatusCode::CONFLICT,"chief-not-current","该会话不是当前有效 chief；旧进程仍存活不授予职责，请由用户明确恢复。".into()),
            Self::Storage(error) => crate::rejected_owned(StatusCode::INTERNAL_SERVER_ERROR,"chief-storage-failed",format!("无法确认 chief 权威状态：{error}")),
        }
    }
}
pub(crate) async fn get_current(State(state):State<AppState>) -> Response {
    let _membership = state.chief.membership.lock().await;
    match state.chief.current() { Ok(current)=>Json(json!({"current":current})).into_response(),Err(error)=>error.response() }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ReleaseRequest { claim_id:String }
pub(crate) async fn release(State(state):State<AppState>,Json(request):Json<ReleaseRequest>) -> Response {
    let _membership=state.chief.membership.lock().await;
    match state.chief.release(&request.claim_id) {
        Ok(released)=>Json(json!({"current":null,"released":released})).into_response(),
        Err(error)=>error.response(),
    }
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct QualifyRequest { session_id:String }
pub(crate) async fn qualify(State(state):State<AppState>,Json(request):Json<QualifyRequest>) -> Response {
    let _membership = state.chief.membership.lock().await;
    match state.chief.qualify(&request.session_id) { Ok(current)=>Json(json!({"current":current})).into_response(),Err(error)=>error.response() }
}
