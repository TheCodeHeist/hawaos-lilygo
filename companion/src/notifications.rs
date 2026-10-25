use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::models::{OutboundAction, SharedState};

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            if ch == '\n' || ch == '\r' || ch == '\t' {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn spawn_notification_listener(state: SharedState) {
    thread::spawn(move || {
        loop {
            let mut child = match Command::new("stdbuf")
                .args([
                    "-oL",
                    "-eL",
                    "dbus-monitor",
                    "interface='org.freedesktop.Notifications',member='Notify'",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => match Command::new("dbus-monitor")
                    .args(["interface='org.freedesktop.Notifications',member='Notify'"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[NotificationListener] Failed to start dbus-monitor: {}", e);
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                },
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut in_notify = false;
                let mut strings: Vec<String> = Vec::new();

                for line_res in reader.lines() {
                    let line = match line_res {
                        Ok(l) => l,
                        Err(_) => break,
                    };

                    if line.contains("member=Notify") {
                        in_notify = true;
                        strings.clear();
                        continue;
                    }

                    if !in_notify {
                        continue;
                    }

                    if line.contains("array [") {
                        if strings.len() >= 4 {
                            let app_name = strip_html_tags(strings[0].trim());
                            let summary = strip_html_tags(strings[2].trim());
                            let body = strip_html_tags(strings[3].trim());

                            if !app_name.to_lowercase().contains("hawaos") {
                                let title = if !summary.is_empty() {
                                    summary.clone()
                                } else if !app_name.is_empty() {
                                    app_name.clone()
                                } else {
                                    "Notification".to_string()
                                };

                                if !title.is_empty() || !body.is_empty() {
                                    let display_app =
                                        if !app_name.is_empty() && app_name != "notify-send" {
                                            app_name.clone()
                                        } else if !summary.is_empty() && !body.is_empty() {
                                            summary.clone()
                                        } else if !app_name.is_empty() {
                                            app_name.clone()
                                        } else {
                                            "Notification".to_string()
                                        };

                                    let display_title = if app_name == "notify-send"
                                        && !summary.is_empty()
                                        && !body.is_empty()
                                    {
                                        summary.clone()
                                    } else if !summary.is_empty() {
                                        summary.clone()
                                    } else if !app_name.is_empty() {
                                        app_name.clone()
                                    } else {
                                        "Notification".to_string()
                                    };

                                    eprintln!(
                                        "[NotificationListener] Captured notification: '{}' | '{}' (app: {})",
                                        display_title, body, display_app
                                    );
                                    if let Ok(mut g) = state.lock() {
                                        let muted = g.stopwatch_state == "running" || g.timer_state == "running";
                                        if muted {
                                            g.push_log(format!(
                                                "[Notify Muted] ({}): {}: {} (clock active)",
                                                display_app, display_title, body
                                            ));
                                            eprintln!("[NotificationListener] Muting notification: timer/stopwatch is running!");
                                        } else {
                                            g.push_log(format!(
                                                "[Notify] ({}): {}: {}",
                                                display_app, display_title, body
                                            ));
                                            g.outbound_queue.push_back(
                                                OutboundAction::SendNotification {
                                                    app: display_app,
                                                    title: display_title,
                                                    body,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        in_notify = false;
                        strings.clear();
                        continue;
                    }

                    let trimmed = line.trim();
                    if trimmed.starts_with("string \"")
                        && trimmed.ends_with('"')
                        && trimmed.len() >= 9
                    {
                        let val = &trimmed[8..trimmed.len() - 1];
                        strings.push(val.to_string());
                    } else if trimmed.starts_with("string \"") {
                        let val = &trimmed[8..];
                        strings.push(val.to_string());
                    } else if !strings.is_empty()
                        && !trimmed.starts_with("uint32")
                        && !trimmed.starts_with("int32")
                        && !trimmed.starts_with("array")
                        && !trimmed.starts_with("struct")
                        && !trimmed.starts_with("dict")
                    {
                        if trimmed.ends_with('"') {
                            if let Some(last) = strings.last_mut() {
                                last.push(' ');
                                last.push_str(&trimmed[..trimmed.len() - 1]);
                            }
                        } else if let Some(last) = strings.last_mut() {
                            last.push(' ');
                            last.push_str(trimmed);
                        }
                    }
                }
            }

            let _ = child.wait();
            thread::sleep(Duration::from_secs(1));
        }
    });
}
