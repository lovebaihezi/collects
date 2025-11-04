use chrono::{DateTime, Utc};
use egui::{Color32, Context, Ui, Widget};
use flume::{Sender, TryRecvError};
use log::info;

pub fn api_status(ctx: &Context, ui: &Ui) -> impl Widget + '_ {
    match is_api_health {
        Some(true) => {
            ui.colored_label(Color32::GREEN, "API Status: Healthy");
        }
        Some(false) => {
            ui.colored_label(Color32::RED, "API Status: Unhealthy");
        }
        None => {
            ui.colored_label(Color32::YELLOW, "API Status: Checking...");
        }
    }
}

pub fn is_api_health(ctx: &Context) -> Option<bool> {
    // first, if we have one res from the channel, which gives the latest status of the API,
    // so update the previous_check_api_status and previous_check_time
    // if not, check the previous_check_time, if it is None or more than 5 minutes ago, send a new check request
    // if not, return the previous_check_api_status, if previous check api status is None, return false and send a new check request
    let cur_time = Utc::now();

    fn send_check_request(cur_time: &DateTime<Utc>, sender: Sender<bool>) {
        info!("Send one check request at {:?}", cur_time);
        let req = ehttp::Request::get("https://collects.lqxclqxc.com/api/is-health");
        ehttp::fetch(req, move |res| match res {
            Ok(res) => {
                sender.send(res.status == 200).unwrap_or(());
            }
            Err(_) => {
                sender.send(false).unwrap_or(());
            }
        });
    }

    match self.recv.try_recv() {
        Ok(res) => {
            self.previous_check_api_status = Some((res, cur_time));
            Some(res)
        }
        Err(TryRecvError::Empty) => match self.previous_check_api_status {
            None => {
                self.previous_check_api_status = Some((false, cur_time));
                send_check_request(&cur_time, self.send.clone());
                None
            }
            Some((previous_status, previous_check_time)) => {
                if (cur_time - previous_check_time).num_minutes() < 5 {
                    Some(previous_status)
                } else {
                    self.previous_check_api_status = Some((previous_status, cur_time));
                    send_check_request(&cur_time, self.send.clone());
                    None
                }
            }
        },
        Err(TryRecvError::Disconnected) => {
            panic!("All Sender got dropped, some bugs exits in code!@")
        }
    }
}

pub fn signin_button(ui: &Ui, ctx: &Context) -> impl Widget + '_ {
    ui.button("Sign In")
}
