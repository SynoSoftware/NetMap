use std::{io, path::Path};

/// Native workspace coordinates, passed back only to SetWindowPlacement.
/// Minimized is a transient state; the next launch restores normal/maximized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placement {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub maximized: bool,
}

impl Placement {
    fn encode(self) -> String {
        format!(
            "NetMap window 1\n{},{},{},{}\n{}\n",
            self.left, self.top, self.right, self.bottom, self.maximized
        )
    }

    fn decode(text: &str) -> io::Result<Self> {
        let invalid =
            || io::Error::new(io::ErrorKind::InvalidData, "Invalid saved window placement");
        let mut lines = text.lines();
        if lines.next() != Some("NetMap window 1") {
            return Err(invalid());
        }
        let mut coordinates = lines.next().ok_or_else(invalid)?.split(',');
        let mut coordinate = || {
            coordinates
                .next()
                .ok_or_else(invalid)?
                .parse::<i32>()
                .map_err(|_| invalid())
        };
        let value = Self {
            left: coordinate()?,
            top: coordinate()?,
            right: coordinate()?,
            bottom: coordinate()?,
            maximized: lines
                .next()
                .ok_or_else(invalid)?
                .parse()
                .map_err(|_| invalid())?,
        };
        if coordinates.next().is_some()
            || lines.next().is_some()
            || value
                .right
                .checked_sub(value.left)
                .is_none_or(|width| width <= 0)
            || value
                .bottom
                .checked_sub(value.top)
                .is_none_or(|height| height <= 0)
        {
            return Err(invalid());
        }
        Ok(value)
    }
}

pub(super) enum Restore {
    Pending,
    Ready(Option<Placement>),
    Applied,
}

pub(super) fn load(directory: &Path) -> io::Result<Option<Placement>> {
    let file = super::storage::File::open(directory, "window")?;
    match file.read() {
        Ok(text) => Placement::decode(&text).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn save(directory: &Path, desired: Placement) -> io::Result<()> {
    let file = super::storage::File::open(directory, "window")?;
    let previous = match file.read() {
        Ok(text) => Placement::decode(&text).unwrap_or(desired),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidData
            ) =>
        {
            desired
        }
        Err(error) => return Err(error),
    };
    file.commit(&previous.encode(), &desired.encode())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_placement_preserves_negative_coordinates_and_rejects_invalid_extents() {
        let value = Placement {
            left: -1800,
            top: -300,
            right: -600,
            bottom: 500,
            maximized: true,
        };
        assert_eq!(Placement::decode(&value.encode()).unwrap(), value);
        assert!(Placement::decode("NetMap window 1\n1,2,1,4\nfalse\n").is_err());
        assert!(Placement::decode("NetMap window 1\n-2147483648,0,2147483647,4\nfalse\n").is_err());
        assert!(Placement::decode(&(value.encode() + "extra\n")).is_err());
    }
}
