// Ported from TinyTorrent2 Geography.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use std::{collections::HashSet, io};
use windows_numerics::Vector2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: Vector2,
    pub level: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub section: usize,
    pub reversed: bool,
    pub draws: bool,
}

#[derive(Debug)]
pub struct Area {
    pub name: Box<str>,
    pub population: f64,
    pub point: Point,
}

#[derive(Debug)]
pub struct Country {
    pub code: Box<str>,
    pub anchor: Point,
    pub rings: Box<[Box<[Part]>]>,
    pub areas: Box<[Area]>,
}

#[derive(Debug)]
pub struct World {
    pub sections: Box<[Box<[Vertex]>]>,
    pub countries: Box<[Country]>,
}

impl World {
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut read = Reader(bytes);
        let mut sections = Vec::new();
        for _ in 0..read.count(4)? {
            let count = read.count(12)?;
            let mut vertices = Vec::with_capacity(count);
            for _ in 0..count {
                let position = Vector2::new(read.finite()?, read.finite()?);
                let level = f32::from_le_bytes(read.take()?);
                if level.is_nan() || level < 0.0 {
                    return Err(invalid("Invalid outline simplification level"));
                }
                vertices.push(Vertex { position, level });
            }
            sections.push(vertices.into_boxed_slice());
        }

        let mut countries = Vec::new();
        let mut codes = HashSet::new();
        for _ in 0..read.count(17)? {
            let code = read.string()?;
            if code.is_empty() || !codes.insert(code.clone()) {
                return Err(invalid("Empty or duplicate country code"));
            }
            let anchor = read.point()?;
            let mut rings = Vec::new();
            for _ in 0..read.count(4)? {
                let count = read.count(5)?;
                let mut parts = Vec::with_capacity(count);
                for _ in 0..count {
                    let section = read.u32()? as usize;
                    let [flags] = read.take()?;
                    if section >= sections.len() || flags & !3 != 0 {
                        return Err(invalid("Invalid shared outline reference"));
                    }
                    parts.push(Part {
                        section,
                        reversed: flags & 1 != 0,
                        draws: flags & 2 != 0,
                    });
                }
                rings.push(parts.into_boxed_slice());
            }
            let mut areas = Vec::new();
            for _ in 0..read.count(13)? {
                let name = read.string()?;
                let population = i32::from_le_bytes(read.take()?);
                if population < 0 {
                    return Err(invalid("Invalid populated-place weight"));
                }
                areas.push(Area {
                    name,
                    population: f64::from(population),
                    point: read.point()?,
                });
            }
            countries.push(Country {
                code,
                anchor,
                rings: rings.into_boxed_slice(),
                areas: areas.into_boxed_slice(),
            });
        }
        if !read.0.is_empty() {
            return Err(invalid("Unexpected trailing geography data"));
        }
        Ok(Self {
            sections: sections.into_boxed_slice(),
            countries: countries.into_boxed_slice(),
        })
    }

    pub fn country(&self, code: &str) -> Option<&Country> {
        self.countries
            .iter()
            .find(|country| country.code.as_ref() == code)
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let Some((bytes, rest)) = self.0.split_first_chunk() else {
            return Err(invalid("Truncated geography data"));
        };
        self.0 = rest;
        Ok(*bytes)
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn count(&mut self, element_bytes: usize) -> io::Result<usize> {
        let count = self.u32()? as usize;
        if count > self.0.len() / element_bytes {
            return Err(invalid("Geography count exceeds remaining data"));
        }
        Ok(count)
    }

    fn finite(&mut self) -> io::Result<f32> {
        let value = f32::from_le_bytes(self.take()?);
        if !value.is_finite() {
            return Err(invalid("Nonfinite geography coordinate"));
        }
        Ok(value)
    }

    fn point(&mut self) -> io::Result<Point> {
        Ok(Point {
            longitude: f64::from(self.finite()?),
            latitude: f64::from(self.finite()?),
        })
    }

    fn string(&mut self) -> io::Result<Box<str>> {
        // BinaryReader strings prefix UTF-8 byte length with a 7-bit integer.
        let mut length = 0usize;
        for shift in (0..35).step_by(7) {
            let [byte] = self.take()?;
            if shift == 28 && byte > 7 {
                return Err(invalid("Invalid geography string length"));
            }
            length |= usize::from(byte & 127) << shift;
            if byte & 128 == 0 {
                let Some((bytes, rest)) = self.0.split_at_checked(length) else {
                    return Err(invalid("Truncated geography string"));
                };
                let value =
                    std::str::from_utf8(bytes).map_err(|_| invalid("Invalid geography UTF-8"))?;
                self.0 = rest;
                return Ok(value.into());
            }
        }
        Err(invalid("Invalid geography string length"))
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_asset_keeps_shared_sections_and_country_precision() {
        let world = World::decode(include_bytes!("../assets/world.bin")).unwrap();
        assert!(world.countries.len() > 200);
        let us = world.country("US").unwrap();
        assert!(!us.rings.is_empty());
        assert!(!us.areas.is_empty());
        assert!(
            us.areas
                .iter()
                .any(|area| !area.name.is_empty() && area.population > 0.0)
        );
        let references: usize = world
            .countries
            .iter()
            .flat_map(|country| &country.rings)
            .map(|ring| ring.len())
            .sum();
        assert!(references > world.sections.len());
        assert!(
            world
                .sections
                .iter()
                .any(|section| section.iter().any(|vertex| vertex.level.is_infinite()))
        );
    }

    #[test]
    fn truncated_asset_is_rejected_before_allocating_from_its_count() {
        assert!(World::decode(&u32::MAX.to_le_bytes()).is_err());
        let original = include_bytes!("../assets/world.bin");
        assert!(World::decode(&original[..original.len() - 1]).is_err());
    }

    #[test]
    fn dotnet_string_prefix_counts_utf8_bytes_and_rejects_overflow() {
        let mut reader = Reader(&[4, 0xc3, 0x85, 0xc3, 0x85]);
        assert_eq!(&*reader.string().unwrap(), "ÅÅ");
        assert!(Reader(&[255, 255, 255, 255, 255]).string().is_err());
        assert!(Reader(&[2, 0xff, 0xff]).string().is_err());
    }
}
