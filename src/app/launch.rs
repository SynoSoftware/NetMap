use std::{ffi::OsString, io, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Window,
    ScreenSaver,
    Preview(usize),
    Configure(Option<usize>),
    Collector,
}

pub(crate) struct Launch {
    pub mode: Mode,
    pub assets: PathBuf,
}

impl Launch {
    pub fn parse(
        arguments: impl IntoIterator<Item = OsString>,
        assets: PathBuf,
    ) -> io::Result<Self> {
        let mut arguments = arguments.into_iter();
        let mut launch = Self {
            mode: Mode::Window,
            assets,
        };
        let Some(first) = arguments.next() else {
            return Ok(launch);
        };
        if first == "--collector" {
            launch.mode = Mode::Collector;
        } else if first == "--assets" {
            launch.assets = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| io::Error::other("--assets requires a directory"))?;
        } else {
            let first = first
                .to_str()
                .ok_or_else(|| io::Error::other("Invalid launch argument"))?;
            let (option, inline) = first
                .split_once(':')
                .map_or((first, None), |(key, value)| (key, Some(value)));
            let option = option.to_ascii_lowercase();
            let owner = |value: &str| -> io::Result<usize> {
                value
                    .parse::<usize>()
                    .ok()
                    .filter(|value| *value != 0 && *value <= isize::MAX as usize)
                    .ok_or_else(|| io::Error::other("Expected a valid parent window handle"))
            };
            launch.mode = match option.as_str() {
                "/s" | "-s" if inline.is_none() => Mode::ScreenSaver,
                "/p" | "-p" => {
                    let value = inline
                        .map(OsString::from)
                        .or_else(|| arguments.next())
                        .ok_or_else(|| io::Error::other("Preview requires a parent window"))?;
                    Mode::Preview(owner(value.to_str().unwrap_or_default())?)
                }
                "/c" | "-c" => {
                    let value = inline.map(OsString::from).or_else(|| arguments.next());
                    Mode::Configure(
                        value
                            .as_ref()
                            .filter(|value| value.as_os_str() != "0")
                            .map(|value| owner(value.to_str().unwrap_or_default()))
                            .transpose()?,
                    )
                }
                _ => {
                    return Err(io::Error::other(
                        "Expected --assets <directory>, /s, /p <HWND>, or /c[:HWND]",
                    ));
                }
            };
        }
        if arguments.next().is_some() {
            return Err(io::Error::other("Unexpected command-line argument"));
        }
        Ok(launch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saver_launch_never_falls_back_to_an_interactive_window() {
        let parse =
            |args: &[&str]| Launch::parse(args.iter().map(OsString::from), PathBuf::from("assets"));
        assert_eq!(parse(&["/S"]).unwrap().mode, Mode::ScreenSaver);
        assert_eq!(parse(&["/p", "123"]).unwrap().mode, Mode::Preview(123));
        assert_eq!(parse(&["/p:123"]).unwrap().mode, Mode::Preview(123));
        assert_eq!(parse(&["/c:123"]).unwrap().mode, Mode::Configure(Some(123)));
        assert_eq!(parse(&["/c"]).unwrap().mode, Mode::Configure(None));
        assert_eq!(parse(&["/c:0"]).unwrap().mode, Mode::Configure(None));
        assert_eq!(parse(&["--collector"]).unwrap().mode, Mode::Collector);
        for invalid in [
            vec!["/p"],
            vec!["/p", "0"],
            vec!["/s:123"],
            vec!["/s", "extra"],
            vec!["/c:bad"],
        ] {
            assert!(parse(&invalid).is_err());
        }
    }
}
