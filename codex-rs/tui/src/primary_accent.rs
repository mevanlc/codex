use ratatui::style::Color;
use std::sync::LazyLock;
use std::sync::RwLock;

static PRIMARY_ACCENT: LazyLock<RwLock<Option<Color>>> = LazyLock::new(|| RwLock::new(None));

const PRIMARY_ACCENT_FORMAT_GUIDANCE: &str =
    "expected r,g,b | #RRGGBB | 0..255 (ANSI palette index)";

/// Parses a `tui.primary_accent` config value into a terminal color.
#[allow(clippy::disallowed_methods)]
pub(crate) fn parse_primary_accent(raw: &str) -> Result<Color, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(format!("empty value ({PRIMARY_ACCENT_FORMAT_GUIDANCE})"));
    }

    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(format!(
                "invalid hex color `{value}` ({PRIMARY_ACCENT_FORMAT_GUIDANCE})"
            ));
        }

        let parse_channel = |start: usize| -> Result<u8, String> {
            u8::from_str_radix(&hex[start..start + 2], 16).map_err(|_| {
                format!("invalid hex color `{value}` ({PRIMARY_ACCENT_FORMAT_GUIDANCE})")
            })
        };

        return Ok(Color::Rgb(
            parse_channel(0)?,
            parse_channel(2)?,
            parse_channel(4)?,
        ));
    }

    if value.contains(',') {
        let pieces: Vec<&str> = value.split(',').map(str::trim).collect();
        if pieces.len() != 3 {
            return Err(format!(
                "invalid RGB tuple `{value}` ({PRIMARY_ACCENT_FORMAT_GUIDANCE})"
            ));
        }

        let parse_channel = |channel: &str| -> Result<u8, String> {
            channel.parse::<u8>().map_err(|_| {
                format!("invalid RGB tuple `{value}` ({PRIMARY_ACCENT_FORMAT_GUIDANCE})")
            })
        };

        return Ok(Color::Rgb(
            parse_channel(pieces[0])?,
            parse_channel(pieces[1])?,
            parse_channel(pieces[2])?,
        ));
    }

    let index = value
        .parse::<u8>()
        .map_err(|_| format!("invalid color `{value}` ({PRIMARY_ACCENT_FORMAT_GUIDANCE})"))?;
    Ok(Color::Indexed(index))
}

pub(crate) fn configure_from_config(raw: Option<&str>) -> Result<(), String> {
    let accent = match raw {
        Some(value) if !value.trim().is_empty() => Some(parse_primary_accent(value)?),
        _ => None,
    };
    set_primary_accent(accent);
    Ok(())
}

pub(crate) fn set_primary_accent(accent: Option<Color>) {
    if let Ok(mut guard) = PRIMARY_ACCENT.write() {
        *guard = accent;
    }
}

pub(crate) fn current_primary_accent() -> Option<Color> {
    PRIMARY_ACCENT.read().ok().and_then(|guard| *guard)
}

pub(crate) fn remap_cyan(color: Color, primary_accent: Option<Color>) -> Color {
    if color == Color::Cyan {
        primary_accent.unwrap_or(Color::Cyan)
    } else {
        color
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_tuple() {
        assert_eq!(
            parse_primary_accent("1,2,3").expect("valid RGB tuple"),
            Color::Rgb(1, 2, 3)
        );
    }

    #[test]
    fn parses_hex() {
        assert_eq!(
            parse_primary_accent("#00AAFF").expect("valid hex"),
            Color::Rgb(0, 170, 255)
        );
    }

    #[test]
    fn parses_index() {
        assert_eq!(
            parse_primary_accent("14").expect("valid index"),
            Color::Indexed(14)
        );
    }

    #[test]
    fn rejects_invalid_value() {
        let err = parse_primary_accent("bogus").expect_err("should reject invalid input");
        assert!(err.contains("expected r,g,b | #RRGGBB | 0..255"));
    }
}
