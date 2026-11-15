use anyhow::{
  Context,
  Result
};

use super::super::{
  App,
  Screen
};
use crate::models::TokenResponse;

impl App {
  pub(crate) fn login(
    &mut self
  ) -> Result<()> {
    let url = format!(
      "{}/v1/auth/login",
      self.base_url
    );

    let body = serde_json::json!({
      "username": &self.username,
      "password": &self.password,
    });

    let resp = self
      .client
      .post(url)
      .json(&body)
      .send()
      .context(
        "login request failed"
      )?;

    if !resp.status().is_success() {
      let msg = resp
        .text()
        .unwrap_or_else(|_| {
          "login failed".to_string()
        });

      self.status =
        format!("Login failed: {msg}");

      return Ok(());
    }

    let token = resp
      .json::<TokenResponse>()?
      .token;

    self.token = Some(token);
    self.screen = Screen::Main;
    self.needs_refresh = true;
    self.status = "Logged in. Loading \
                   feeds..."
      .to_string();

    Ok(())
  }

  /// Revoke the current bearer before
  /// exiting the interactive client.
  /// Exiting avoids applying late
  /// background events to a subsequent
  /// login session.
  pub(crate) fn logout_and_exit(
    &mut self
  ) -> bool {
    let Some(token) =
      self.token.as_deref()
    else {
      return true;
    };

    let response = self
      .client
      .post(format!(
        "{}/v1/auth/logout",
        self.base_url
      ))
      .bearer_auth(token)
      .send();

    match response {
      | Ok(resp)
        if resp.status().as_u16()
          == 204
          || resp.status().as_u16()
            == 401 =>
      {
        self.token = None;
        self.password.clear();
        self.status =
          "Logged out.".to_string();
        true
      }
      | Ok(resp) => {
        self.status = format!(
          "Logout failed ({})",
          resp.status()
        );
        false
      }
      | Err(_) => {
        self.status =
          "Logout request failed;            still signed in."
            .to_string();
        false
      }
    }
  }
}

#[cfg(test)]
mod logout_tests {
  use std::io::{
    BufRead,
    BufReader,
    Write
  };
  use std::net::TcpListener;
  use std::thread;
  use std::time::Duration;

  use super::*;
  use crate::config::TuiConfig;

  #[test]
  fn failed_logout_preserves_authenticated_session()
   {
    let listener =
      TcpListener::bind("127.0.0.1:0")
        .expect(
          "isolated logout listener"
        );
    let address = listener
      .local_addr()
      .expect("listener address");
    let worker =
      thread::spawn(move || {
        for status in [
          "503 Service Unavailable",
          "204 No Content"
        ] {
          let (mut socket, _) =
            listener
              .accept()
              .expect("logout request");
          socket
            .set_read_timeout(Some(
              Duration::from_secs(5)
            ))
            .expect("read timeout");
          let mut reader =
            BufReader::new(&socket);
          let mut line = String::new();
          reader
            .read_line(&mut line)
            .expect(
              "read logout request"
            );
          assert!(
            line.contains(
              "/v1/auth/logout"
            ),
            "unexpected logout path"
          );
          loop {
            let mut header =
              String::new();
            let n = reader
              .read_line(&mut header)
              .expect("read header");
            if n == 0
              || header == "\r\n"
              || header == "\n"
            {
              break;
            }
          }
          drop(reader);
          let response = [
            format!(
              "HTTP/1.1 {status}\r\n"
            ),
            "Content-Length: 0\r\n"
              .to_string(),
            "Connection: close\r\n"
              .to_string(),
            "\r\n".to_string()
          ]
          .concat();
          socket
            .write_all(
              response.as_bytes()
            )
            .expect(
              "send logout response"
            );
        }
      });

    let config: TuiConfig =
      toml::from_str(include_str!(
        "../../../res/config.toml"
      ))
      .expect("fixture config");
    let keys = config
      .resolved_keybindings()
      .expect("fixture keys");
    let mut app =
      App::new(&config, keys)
        .expect("offline TUI app");
    app.base_url =
      format!("http://{address}");
    app.token =
      Some("synthetic-token".into());
    app.password =
      "synthetic-password".into();

    assert!(!app.logout_and_exit());
    assert_eq!(
      app.token.as_deref(),
      Some("synthetic-token")
    );
    assert!(app.status.contains("503"));
    assert!(app.logout_and_exit());
    assert!(app.token.is_none());
    assert!(app.password.is_empty());
    worker
      .join()
      .expect("logout server");
  }
}
