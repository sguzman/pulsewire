//! Deterministic client-only journey
//! against the disposable SQL
//! server created by CI.
//! This is intentionally ignored in
//! ordinary developer test runs and
//! must never target a household server
//! or real credentials.

use std::sync::mpsc::{
  self,
  Receiver
};
use std::time::Duration;

use crossterm::event::KeyEvent;

use super::{
  App,
  AppEvent,
  Screen
};
use crate::config::{
  KeyBinding,
  TuiConfig
};

const CI_FEED: &str =
  "pulsewire-ci-real-rss";

fn receive(
  app: &mut App,
  rx: &Receiver<AppEvent>,
  expected: fn(&AppEvent) -> bool
) {
  let event = rx
    .recv_timeout(Duration::from_secs(
      10
    ))
    .expect("missing TUI HTTP event");
  assert!(
    expected(&event),
    "unexpected TUI event variant"
  );
  app.apply_event(event);
  assert!(
    app.error.is_none(),
    "TUI operation failed"
  );
}

fn press(
  app: &mut App,
  binding: KeyBinding
) {
  let key = KeyEvent::new(
    binding.code,
    binding.modifiers
  );
  assert!(
    !app
      .handle_key(key)
      .expect("handle scripted key"),
    "client unexpectedly quit"
  );
}

#[test]
#[ignore = "requires isolated CI \
            PostgreSQL HTTP fixture"]
fn isolated_http_tui_reader_journey() {
  let url = std::env::var(
    "PULSEWIRE_TUI_E2E_URL"
  )
  .expect("CI server URL required");
  let password = std::env::var(
    "PULSEWIRE_TUI_E2E_PASSWORD"
  )
  .expect("CI password required");
  let config: TuiConfig =
    toml::from_str(include_str!(
      "../../res/config.toml"
    ))
    .expect("CI TUI config");
  let keys = config
    .resolved_keybindings()
    .expect("CI TUI keybindings");
  let mut app = App::new(&config, keys)
    .expect("create TUI client");
  app.base_url = url.clone();
  app.username = "owner".to_string();

  // Incorrect credentials must leave
  // the client on the login screen.
  app.password =
    "ci-incorrect-password".into();
  app
    .login()
    .expect("invalid login response");
  assert_eq!(app.screen, Screen::Login);
  assert!(app.token.is_none());

  app.password = password.clone();
  app.login().expect("valid CI login");
  assert_eq!(app.screen, Screen::Main);
  assert!(app.token.is_some());

  // Real HTTP data was ingested by the
  // separate Docker fetcher fixture.
  app
    .refresh_feeds()
    .expect("TUI feeds");
  assert!(
    app
      .feeds
      .iter()
      .any(|feed| feed.id == CI_FEED)
  );
  app.entries_page_size = 1;
  app
    .open_all_entries()
    .expect("TUI first page");
  assert_eq!(app.entries.len(), 1);
  let first = app.entries[0].id;
  let first_title =
    app.entries[0].title.clone();
  assert!(
    app.entries_next_offset.is_some()
  );
  app
    .next_entries_page()
    .expect("TUI second page");
  assert_eq!(app.entries.len(), 1);
  assert_ne!(app.entries[0].id, first);
  let second_title =
    app.entries[0].title.clone();
  app.entries_page_size = 50;
  app
    .open_search_entries(
      "Pulsewire CI Story".into()
    )
    .expect("TUI search");
  assert!(
    app.status.starts_with("Loaded "),
    "search did not succeed: {}",
    app.status
  );
  assert!(
    app.entries.len() == 2
      && app.entries.iter().any(|item| {
        item.title.as_deref()
          == Some("Pulsewire CI Story One")
      })
      && app.entries.iter().any(|item| {
        item.title.as_deref()
          == Some("Pulsewire CI Story Two")
      }),
    "search count: {}; first page: \
     {:?}; second page: {:?}; search \
     URL: {:?}",
    app.entries.len(),
    first_title,
    second_title,
    app.entries_url()
  );

  let (tx, rx) = mpsc::channel();
  app.set_event_sender(tx);

  app
    .refresh_subscriptions()
    .expect("TUI subscriptions");
  app
    .refresh_favorites()
    .expect("TUI favorites");
  let idx = app
    .feeds_view
    .iter()
    .position(|index| {
      app.feeds[*index].id == CI_FEED
    })
    .expect("CI feed visible");
  app.selected_feed = idx;
  app.tab = 0;

  // Exercise actual keyboard handlers
  // and asynchronous client mutation
  // paths.
  let subscribe_key =
    app.keys.toggle_subscribe.clone();
  let subscribed =
    app.subscriptions.contains(CI_FEED);
  press(
    &mut app,
    subscribe_key.clone()
  );
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleSubscribe {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_subscriptions()
    .expect("changed subscription");
  assert_ne!(
    app.subscriptions.contains(CI_FEED),
    subscribed
  );
  press(
    &mut app,
    subscribe_key.clone()
  );
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleSubscribe {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_subscriptions()
    .expect("restored subscription");
  assert_eq!(
    app.subscriptions.contains(CI_FEED),
    subscribed
  );

  let favorite_key =
    app.keys.toggle_favorite.clone();
  let favorite =
    app.favorite_ids.contains(CI_FEED);
  press(&mut app, favorite_key.clone());
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleFavorite {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_favorites()
    .expect("changed favorites");
  assert_ne!(
    app.favorite_ids.contains(CI_FEED),
    favorite
  );
  press(&mut app, favorite_key.clone());
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleFavorite {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_favorites()
    .expect("restored favorites");
  assert_eq!(
    app.favorite_ids.contains(CI_FEED),
    favorite
  );

  let read_key =
    app.keys.toggle_read.clone();
  app
    .open_all_entries()
    .expect("read-state list");
  let original_read =
    app.entries[0].is_read;
  let item = app.entries[0].id;
  press(&mut app, read_key.clone());
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleEntryRead {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_entries()
    .expect("changed read state");
  assert_eq!(app.entries[0].id, item);
  assert_ne!(
    app.entries[0].is_read,
    original_read
  );
  press(&mut app, read_key.clone());
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::ToggleEntryRead {
        ok: true,
        ..
      }
    )
  });
  app
    .refresh_entries()
    .expect("restored read state");
  assert_eq!(
    app.entries[0].is_read,
    original_read
  );

  // Folder state must travel through
  // the TUI worker, HTTP server and
  // database.
  let folder_name =
    "PW051 CI TUI Reader";
  app.queue_create_folder(
    folder_name.to_string()
  );
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::RefreshFolders { .. }
    )
  });
  let folder_index = app
    .folders
    .iter()
    .position(|row| {
      row.name == folder_name
    })
    .expect("created folder in TUI");
  let folder_id =
    app.folders[folder_index].id;
  app.selected_folder = folder_index;
  app.tab = 3;
  // RefreshFolders schedules a
  // folder-feed request; consume it
  // before assignment.
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::FolderFeeds { .. }
    )
  });
  app.queue_assign_folder_feed(
    folder_id,
    CI_FEED.to_string()
  );
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::FolderFeedUpdate {
        ok: true,
        ..
      }
    )
  });
  app.request_folder_feeds();
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::FolderFeeds { .. }
    )
  });
  assert!(
    app
      .folder_feed_ids
      .iter()
      .any(|feed| feed == CI_FEED)
  );

  // Verify server-side token revocation
  // and a newly created TUI session.
  let token = app
    .token
    .as_ref()
    .expect("authenticated token");
  let logout = app
    .client
    .post(format!(
      "{url}/v1/auth/logout"
    ))
    .bearer_auth(token)
    .send()
    .expect("server logout");
  assert!(logout.status().is_success());
  let revoked = app
    .client
    .get(format!("{url}/v1/entries"))
    .bearer_auth(token)
    .send()
    .expect("revoked token response");
  assert_eq!(
    revoked.status().as_u16(),
    401
  );

  let mut resumed = App::new(
    &config,
    config
      .resolved_keybindings()
      .expect("second TUI keys")
  )
  .expect("restart client");
  resumed.base_url = url;
  resumed.username = "owner".into();
  resumed.password = password;
  resumed
    .login()
    .expect("client re-login");
  resumed
    .refresh_feeds()
    .expect("feeds after re-login");
  resumed
    .refresh_subscriptions()
    .expect(
      "subscriptions after re-login"
    );
  resumed
    .refresh_favorites()
    .expect("favorites after re-login");
  resumed
    .refresh_folders()
    .expect("folders after re-login");
  assert!(resumed.folders.iter().any(
    |folder| folder.id == folder_id
  ));
  assert_eq!(
    resumed
      .subscriptions
      .contains(CI_FEED),
    subscribed
  );
  assert_eq!(
    resumed
      .favorite_ids
      .contains(CI_FEED),
    favorite
  );
}

#[test]
#[ignore = "requires restarted CI \
            PostgreSQL HTTP server"]
fn isolated_http_tui_after_restart() {
  let url = std::env::var(
    "PULSEWIRE_TUI_E2E_URL"
  )
  .expect("CI server URL required");
  let password = std::env::var(
    "PULSEWIRE_TUI_E2E_PASSWORD"
  )
  .expect("CI password required");
  let config: TuiConfig =
    toml::from_str(include_str!(
      "../../res/config.toml"
    ))
    .expect("CI TUI config");
  let keys = config
    .resolved_keybindings()
    .expect("CI TUI keys");
  let mut app = App::new(&config, keys)
    .expect("restarted TUI client");
  app.base_url = url;
  app.username = "owner".into();
  app.password = password;
  app.login().expect(
    "login after server restart"
  );
  app
    .refresh_feeds()
    .expect("feeds after restart");
  app.refresh_subscriptions().expect(
    "subscriptions after restart"
  );
  app
    .refresh_favorites()
    .expect("favorites after restart");
  app
    .refresh_folders()
    .expect("folders after restart");
  let index = app
    .folders
    .iter()
    .position(|folder| {
      folder.name
        == "PW051 CI TUI Reader"
    })
    .expect(
      "folder persisted across restart"
    );
  app.selected_folder = index;
  app.tab = 3;

  let (tx, rx) = mpsc::channel();
  app.set_event_sender(tx);
  app.request_folder_feeds();
  receive(&mut app, &rx, |event| {
    matches!(
      event,
      AppEvent::FolderFeeds { .. }
    )
  });
  assert!(
    app
      .folder_feed_ids
      .iter()
      .any(|feed| feed == CI_FEED)
  );
  app
    .open_all_entries()
    .expect("entries after restart");
  assert!(app.entries.len() >= 2);
}
