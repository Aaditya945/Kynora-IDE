use std::{
    collections::{
        HashMap,
        HashSet,
    },
    path::PathBuf,
    process::{
        Child,
        ChildStdin,
    },
    sync::{
        mpsc,
        Arc,
        Mutex,
    },
    time::Instant,
};

use serde_json::Value;
use tauri::ipc::Channel;

use crate::jdtls::JdtlsDiagnosticsEvent;

pub(crate) struct PendingRequest {
    pub(crate) sender:
        mpsc::Sender<Value>,

    pub(crate) method:
        String,

    pub(crate) started_at:
        Instant,
}

pub(crate) type PendingRequests =
    HashMap<
        u64,
        PendingRequest,
    >;

pub(crate) struct JdtlsSession {
    pub(crate) stdin:
        Arc<Mutex<ChildStdin>>,

    pub(crate) child:
        Arc<Mutex<Child>>,

    pub(crate) pending:
        Arc<Mutex<PendingRequests>>,

    pub(crate) cancelled_requests:
        Arc<Mutex<HashSet<u64>>>,

    pub(crate) next_request_id:
        u64,

    pub(crate) root:
        PathBuf,

    pub(crate) data_dir:
        PathBuf,

    pub(crate) document_versions:
        HashMap<String, u32>,

    pub(crate) diagnostics_channel:
        Arc<
            Mutex<
                Option<
                    Channel<
                        JdtlsDiagnosticsEvent,
                    >,
                >,
            >,
        >,
}

#[derive(Default)]
pub struct JdtlsState {
    pub(crate) session:
        Mutex<Option<JdtlsSession>>,
}