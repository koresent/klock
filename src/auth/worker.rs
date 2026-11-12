use pam_client::{Context, ConversationHandler, ErrorCode, Flag};
use std::ffi::{CStr, CString};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    Idle,
    Verifying,
    Success,
    Failure,
}

pub enum AuthRequest {
    Authenticate {
        username: String,
        password: Zeroizing<String>,
    },
    Shutdown,
}

pub enum AuthResponse {
    Success,
    Failure(String),
}

struct StaticConversation {
    password: Zeroizing<String>,
}

impl Zeroize for StaticConversation {
    fn zeroize(&mut self) {
        self.password.zeroize();
    }
}

impl Drop for StaticConversation {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ConversationHandler for StaticConversation {
    fn prompt_echo_off(&mut self, _prompt: &CStr) -> Result<CString, ErrorCode> {
        CString::new(self.password.as_str()).map_err(|_| ErrorCode::CONV_ERR)
    }

    fn prompt_echo_on(&mut self, _prompt: &CStr) -> Result<CString, ErrorCode> {
        CString::new(self.password.as_str()).map_err(|_| ErrorCode::CONV_ERR)
    }

    fn text_info(&mut self, _msg: &CStr) {}
    fn error_msg(&mut self, _msg: &CStr) {}
}

pub struct AuthWorker {
    request_tx: Sender<AuthRequest>,
    response_rx: Receiver<AuthResponse>,
}

impl AuthWorker {
    pub fn spawn(mock: bool) -> Self {
        let (req_tx, req_rx) = channel::<AuthRequest>();
        let (res_tx, res_rx) = channel::<AuthResponse>();

        thread::Builder::new()
            .name("klock-auth-worker".into())
            .spawn(move || {
                while let Ok(msg) = req_rx.recv() {
                    match msg {
                        AuthRequest::Shutdown => break,
                        AuthRequest::Authenticate { username, password } => {
                            if mock {
                                thread::sleep(std::time::Duration::from_millis(150));
                                if password.as_str() == "test"
                                    || password.as_str() == "mock"
                                    || password.is_empty()
                                {
                                    let _ = res_tx.send(AuthResponse::Success);
                                } else {
                                    let _ = res_tx.send(AuthResponse::Failure(
                                        "Invalid test credentials".into(),
                                    ));
                                }
                                continue;
                            }

                            let service = if std::path::Path::new("/etc/pam.d/klock").exists() {
                                "klock"
                            } else if std::path::Path::new("/etc/pam.d/system-auth").exists() {
                                "system-auth"
                            } else {
                                "system-local-login"
                            };

                            let conv = StaticConversation { password };
                            let mut ctx = match Context::new(service, Some(&username), conv) {
                                Ok(c) => c,
                                Err(err) => {
                                    let _ = res_tx.send(AuthResponse::Failure(format!(
                                        "PAM context error: {:?}",
                                        err
                                    )));
                                    continue;
                                }
                            };

                            match ctx.authenticate(Flag::empty()) {
                                Ok(()) => {
                                    let _ = ctx.acct_mgmt(Flag::empty());
                                    let _ = res_tx.send(AuthResponse::Success);
                                }
                                Err(err) => {
                                    let _ = res_tx.send(AuthResponse::Failure(format!(
                                        "Auth failed: {:?}",
                                        err
                                    )));
                                }
                            }
                        }
                    }
                }
            })
            .expect("Failed to spawn auth worker thread");

        Self {
            request_tx: req_tx,
            response_rx: res_rx,
        }
    }

    pub fn submit(&self, username: String, password: Zeroizing<String>) {
        let _ = self
            .request_tx
            .send(AuthRequest::Authenticate { username, password });
    }

    pub fn try_recv(&self) -> Option<AuthResponse> {
        self.response_rx.try_recv().ok()
    }
}

impl Drop for AuthWorker {
    fn drop(&mut self) {
        let _ = self.request_tx.send(AuthRequest::Shutdown);
    }
}
