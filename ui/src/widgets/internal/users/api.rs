//! API calls for internal users management.

use collects_business::{
    DeleteUserResponse, GetUserResponse, ListUsersResponse, RevokeOtpResponse,
    UpdateUsernameResponse,
};

/// Fetch users from the internal API.
pub fn fetch_users(api_base_url: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users");
    let request = ehttp::Request::get(&url);

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(list_response) =
                        serde_json::from_slice::<ListUsersResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("internal_users_response"),
                                list_response.users,
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("internal_users_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("internal_users_error"), err.to_string());
                });
            }
        }
    });
}

/// Fetch user QR code from the internal API.
pub fn fetch_user_qr_code(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let request = ehttp::Request::get(&url);

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(user_response) =
                        serde_json::from_slice::<GetUserResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("user_qr_code_response"),
                                user_response.otpauth_url,
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Update username via the internal API.
pub fn update_username(
    api_base_url: &str,
    old_username: &str,
    new_username: &str,
    ctx: egui::Context,
) {
    let url = format!("{api_base_url}/internal/users/{old_username}");
    let body = serde_json::json!({ "new_username": new_username }).to_string();
    let request = ehttp::Request {
        method: "PUT".to_string(),
        url,
        body: body.into_bytes(),
        headers: ehttp::Headers::new(&[("Content-Type", "application/json")]),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if serde_json::from_slice::<UpdateUsernameResponse>(&response.bytes).is_ok() {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_success"),
                                "username_updated".to_string(),
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Delete user via the internal API.
pub fn delete_user(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let request = ehttp::Request {
        method: "DELETE".to_string(),
        url,
        body: Vec::new(),
        headers: ehttp::Headers::default(),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(delete_response) =
                        serde_json::from_slice::<DeleteUserResponse>(&response.bytes)
                    {
                        if delete_response.deleted {
                            ctx.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("action_success"),
                                    "user_deleted".to_string(),
                                );
                            });
                        } else {
                            ctx.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("action_error"),
                                    "User not found".to_string(),
                                );
                            });
                        }
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Revoke OTP via the internal API.
pub fn revoke_otp(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}/revoke");
    let request = ehttp::Request {
        method: "POST".to_string(),
        url,
        body: Vec::new(),
        headers: ehttp::Headers::default(),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(revoke_response) =
                        serde_json::from_slice::<RevokeOtpResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("revoke_otp_response"),
                                revoke_response.otpauth_url,
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}
