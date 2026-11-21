use jeko::config::{self, Config};
use jeko::content;
use jeko::game::Game;
use jeko::llm::cerebras_client::CerebrasClient;
use jeko::ui;
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: format!("JEKO — Math Quest [{}]", config::instance_name()),
        window_width: 960,
        window_height: 600,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let cfg = Config::load();
    eprintln!(
        "jeko instance {} (built from {})",
        config::instance_name(),
        env!("CARGO_MANIFEST_DIR")
    );
    if cfg.api_key.is_none() {
        eprintln!("CEREBRAS_API_KEY not set — running on the local fallback task bank only.");
    }

    // Dev aid: `JEKO_DEV=fakemodel:700` answers every model call from the
    // offline bank after 700 ms, so the question buffer, the soft deadline and
    // the token panel's wait readout can be tried with no key.
    let fake_ms: Option<u64> = std::env::var("JEKO_DEV")
        .unwrap_or_default()
        .split(',')
        .find_map(|e| e.strip_prefix("fakemodel:")?.parse().ok());
    let mut client = CerebrasClient::new(&cfg);
    if let Some(ms) = fake_ms {
        eprintln!("JEKO_DEV: fake model, {ms} ms per call");
        client = client.with_fake_model(std::time::Duration::from_millis(ms));
    }
    let fallback_tasks = jeko::llm::fallback_bank();
    let mut game = Game::new(client, fallback_tasks);
    let content_dir = content::content_dir();
    eprintln!("content dir {}", content_dir.display());
    game.watch_content(content::watcher::spawn(content_dir));

    // Dev aid: `JEKO_SHOT=out.png:60` saves a screenshot at frame 60 and
    // exits, so a change can be checked without driving the window.
    let shot: Option<(String, u32)> = std::env::var("JEKO_SHOT").ok().and_then(|v| {
        let (path, frame) = v.rsplit_once(':')?;
        Some((path.to_string(), frame.parse().ok()?))
    });
    let mut frame: u32 = 0;
    // Dev aid: `JEKO_DEV=hurt@150,jump@200` fires scripted events at those
    // frames (hurt, kill, jump, roll), for checking effects by screenshot.
    let dev_events: Vec<(String, u32)> = std::env::var("JEKO_DEV")
        .unwrap_or_default()
        .split(',')
        .filter_map(|e| {
            let (name, at) = e.split_once('@')?;
            Some((name.to_string(), at.parse().ok()?))
        })
        .collect();

    loop {
        clear_background(jeko::game::gfx::NIGHT);

        for (name, _) in dev_events.iter().filter(|(_, at)| *at == frame) {
            game.dev_event(name);
        }
        // Scripted dev runs step a fixed 1/60 s so frame numbers map to
        // game time whatever the frame rate.
        let dt = if shot.is_some() || !dev_events.is_empty() {
            1.0 / 60.0
        } else {
            get_frame_time()
        };
        game.update(dt);
        game.draw();
        game.draw_pause_overlay();

        egui_macroquad::ui(|ctx| {
            ui::style::apply(ctx);
            game.typing = ctx.wants_keyboard_input();
            // A battle is its own full-screen view; the side panels would
            // cover the monster.
            let in_battle = game.battle.is_some() || game.dialogue.is_some();
            if game.settings.show_hud && !in_battle {
                ui::hud::draw(ctx, &game.progression, game.distance_m());
            }
            // The goalpost tracker (FR31) and the toast share the top centre,
            // so the toast steps down while the tracker is up.
            let tracker = if game.game_over.is_none() && !in_battle {
                game.goalpost.view()
            } else {
                None
            };
            if let Some(v) = &tracker {
                ui::goalpost_panel::draw(ctx, v);
            }
            let toast_top = if tracker.is_some() {
                ui::goalpost_panel::TOAST_BELOW_BADGE
            } else {
                12.0
            };
            ui::hud::draw_toast(ctx, &game.toast, toast_top);
            if game.settings.show_token_panel && !in_battle {
                ui::token_panel::draw(ctx, game.llm(), &game.waits);
            }
            if let Some(d) = &game.dialogue {
                ui::dialogue_panel::draw(ctx, d);
            }
            if game.can_restart() && ui::game_over_panel::draw(ctx, &game) {
                game.reset();
            }
            if game.settings_open {
                ui::settings_panel::draw(ctx, &mut game.settings, &mut game.settings_open);
            }
            if game.game_over.is_none() {
                if let Some(ui::task_panel::TaskPanelEvent::Submit) = ui::task_panel::draw(
                    ctx,
                    &game.trigger_state,
                    &mut game.input_buffer,
                    &mut game.focus_answer,
                    game.battle.is_some(),
                ) {
                    game.submit_answer();
                }
            }
        });

        egui_macroquad::draw();

        frame += 1;
        if let Some((path, at)) = &shot {
            if frame == *at {
                // Translucent draws (glows, mist, the night vignette) leave
                // the framebuffer's alpha below 1; the window ignores it, so
                // save the shot opaque to match what's on screen.
                let mut img = get_screen_data();
                for px in img.bytes.as_chunks_mut::<4>().0 {
                    px[3] = 255;
                }
                img.export_png(path);
                eprintln!("{}", game.debug_summary());
                return;
            }
        }

        next_frame().await;
    }
}
