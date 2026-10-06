use axum::extract::State;
use maud::{DOCTYPE, Markup, html};

use crate::monitor::{Health, Monitor, history::HOURS};
use crate::state::AppState;

const BRAND: &str = "システム";

fn layout(body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (BRAND) }
                link rel="stylesheet" href="/styles.css";
            }
            body class="min-h-screen bg-brand font-sans text-white antialiased" {
                (body)
            }
        }
    }
}

pub async fn index(State(state): State<AppState>) -> Markup {
    layout(html! {
        @if let Some(banner) = &state.banner {
            div class="fixed inset-x-0 top-0 z-10 bg-notice px-4 py-2 text-center text-sm font-bold text-black" {
                (banner)
            }
        }
        main class="mx-auto max-w-2xl px-4 py-12 sm:py-20" {
            p class="text-center text-6xl font-black tracking-tight [text-shadow:5px_5px_0_var(--color-black)] sm:text-7xl" {
                (BRAND)
            }
            section class="mt-10 overflow-hidden rounded-xl border-[6px] border-black bg-white font-bold text-black shadow-extrude" {
                header class="border-b-2 border-black px-5 py-5" {
                    h1 class="text-2xl font-black" { (state.name) }
                    @if let Some(description) = &state.description {
                        p class="mt-1 font-medium text-black/70" { (description) }
                    }
                }
                ul class="divide-y-2 divide-black/10" {
                    @for monitor in state.monitors.iter() {
                        (service_row(monitor))
                    }
                }
            }
        }
    })
}

fn service_row(monitor: &Monitor) -> Markup {
    let check = monitor.last();
    let health = check.as_ref().map_or(Health::Pending, |check| check.health);

    html! {
        li class="px-5 py-4" {
          div class="flex items-center gap-4" {
            div class="min-w-0 flex-1" {
                p class="text-lg font-black" { (monitor.config.name) }
                p class="truncate text-sm text-black/70" { (monitor.config.url) }
            }
            div class="flex flex-col items-end gap-1 text-right text-sm" {
                span class={ "rounded-md border-[3px] border-black px-2 py-0.5 font-black text-black shadow-hard-sm " (health_bg(health)) } {
                    @if let Some(check) = &check {
                        (check.detail)
                        @if let Some(latency) = check.latency {
                            " · " (latency.as_millis()) " ms"
                        }
                    } @else {
                        "pending"
                    }
                }
            }
          }
          (history_bar(monitor))
        }
    }
}

fn history_bar(monitor: &Monitor) -> Markup {
    html! {
        div class="mt-3" {
            div class="flex h-6 divide-x-2 divide-black overflow-hidden rounded-md border-2 border-black" {
                @for (i, health) in monitor.history().into_iter().enumerate() {
                    @let hours_ago = HOURS - 1 - i;
                    @let when = if hours_ago == 0 { "This hour".to_string() } else { format!("{hours_ago}h ago") };
                    @if let Some(health) = health {
                        span
                            class={ "flex-1 " (health_bg(health)) }
                            title={ (when) ": " (health_label(health)) } {}
                    } @else {
                        span
                            class="flex-1 bg-black/5"
                            title={ (when) ": no data" } {}
                    }
                }
            }
            div class="mt-1 flex justify-between text-xs text-black/60" {
                span { "24h ago" }
                span { "now" }
            }
        }
    }
}

fn health_bg(health: Health) -> &'static str {
    match health {
        Health::Pending => "bg-pending",
        Health::Up => "bg-up",
        Health::Slow => "bg-slow",
        Health::Down => "bg-down",
    }
}

fn health_label(health: Health) -> &'static str {
    match health {
        Health::Pending => "pending",
        Health::Up => "up",
        Health::Slow => "slow",
        Health::Down => "down",
    }
}
