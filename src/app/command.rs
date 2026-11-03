//! Command keys follow the browser host, not WASM's unknown target OS.

use iced::keyboard::Modifiers;

pub(super) fn pressed(modifiers: Modifiers) -> bool {
    pressed_on_platform(modifiers, is_apple())
}

pub(super) fn label() -> &'static str {
    if is_apple() { "Cmd" } else { "Ctrl" }
}

fn pressed_on_platform(modifiers: Modifiers, apple: bool) -> bool {
    if apple {
        modifiers.logo()
    } else {
        modifiers.control()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn is_apple() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export function is_apple() {
    const navigator = globalThis.navigator;
    return /Mac|iPhone|iPad|iPod/.test(
        navigator?.userAgentData?.platform ?? navigator?.platform ?? '');
}
")]
extern "C" {
    fn is_apple() -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_hosts_use_meta_and_other_hosts_use_control() {
        assert!(pressed_on_platform(Modifiers::LOGO, true));
        assert!(pressed_on_platform(
            Modifiers::LOGO | Modifiers::SHIFT,
            true
        ));
        assert!(!pressed_on_platform(Modifiers::CTRL, true));
        assert!(pressed_on_platform(Modifiers::CTRL, false));
        assert!(!pressed_on_platform(Modifiers::LOGO, false));
        assert!(!pressed_on_platform(Modifiers::empty(), true));
        assert!(!pressed_on_platform(Modifiers::empty(), false));
    }
}
