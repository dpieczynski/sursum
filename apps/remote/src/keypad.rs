// SPDX-License-Identifier: GPL-3.0-or-later
//! The "#" keypad: jump to item N of the set with one hand (e.g. the priest announces "pieśń
//! numer 4 z zestawu"). Big keys, no system keyboard needed (which a paired pedal may hide).

use leptos::prelude::*;
use sursum_protocol::Command;

use crate::Commands;
use crate::messages::{Msg, pl};

/// Longest number accepted (sets never have a thousand items).
const MAX_DIGITS: usize = 3;

/// The "#" button and, while open, the keypad.
#[component]
pub fn JumpButton(commands: Commands) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <button class="jump" data-testid="jump" on:click=move |_| open.set(true)>
            "#"
        </button>
        <Show when=move || open.get()>
            <Keypad open commands />
        </Show>
    }
}

#[component]
fn Keypad(open: RwSignal<bool>, commands: Commands) -> impl IntoView {
    let typed = RwSignal::new(String::new());
    let digit_key = move |digit: char| {
        let press = move |_| typed.update(|typed| push_digit(typed, digit));
        view! {
            <button class="key" on:click=press>
                {digit.to_string()}
            </button>
        }
    };
    let digits = ['1', '2', '3', '4', '5', '6', '7', '8', '9'].into_iter().map(digit_key).collect_view();
    let erase = move |_| typed.update(|typed| _ = typed.pop());
    let go = move |_| {
        if let Some(number) = item_number(&typed.get_untracked()) {
            commands.send(Command::GoToItemNumber { number });
        }
        open.set(false);
    };
    let shown = move || {
        let typed = typed.get();
        if typed.is_empty() { pl(Msg::ItemNumber).to_owned() } else { typed }
    };
    view! {
        <div class="keypad" data-testid="keypad">
            <div class="keypad-display">{shown}</div>
            <div class="keypad-keys">
                {digits} <button class="key" on:click=erase>
                    "⌫"
                </button> {digit_key('0')} <button class="key go" data-testid="keypad-go" on:click=go>
                    {pl(Msg::ShowNow)}
                </button>
            </div>
            <button class="keypad-cancel" on:click=move |_| open.set(false)>
                {pl(Msg::Cancel)}
            </button>
        </div>
    }
}

/// Appends a digit, ignoring leading zeros and overly long numbers.
fn push_digit(typed: &mut String, digit: char) {
    if typed.len() < MAX_DIGITS && !(typed.is_empty() && digit == '0') {
        typed.push(digit);
    }
}

/// The 1-based item number, if something was typed.
fn item_number(typed: &str) -> Option<u32> {
    typed.parse().ok().filter(|number| *number > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_make_a_number_without_leading_zeros() {
        let mut typed = String::new();
        for digit in ['0', '1', '2', '3', '4'] {
            push_digit(&mut typed, digit);
        }
        assert_eq!(typed, "123");
        assert_eq!(item_number(&typed), Some(123));
        assert_eq!(item_number(""), None);
    }
}
