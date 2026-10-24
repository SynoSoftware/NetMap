// Ported from TinyTorrent2 MapModel.cs at 95be5335eff5261d4f94b2e77a4bc0db0dcbf151.
// This Source Code Form is subject to the Mozilla Public License, v. 2.0.
// If a copy was not distributed with this file, obtain one at https://mozilla.org/MPL/2.0/.

use super::viewport::wrap;
use crate::{
    connections::{Observation, ObservationId, Rates},
    geography::{Area, Point, World},
    geolocation::Location,
};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use windows_numerics::Vector2;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub code: Box<str>,
    coordinates: Option<(u64, u64)>,
    pub cities: bool,
}

fn coordinates(point: Point) -> (u64, u64) {
    // Numeric equality treats both signs of zero as the same coordinate.
    let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
    (bits(point.longitude), bits(point.latitude))
}

impl Place {
    fn new(location: &Location, cities: bool) -> Self {
        Self {
            code: location.code.clone(),
            coordinates: if cities {
                location.point.map(coordinates)
            } else {
                None
            },
            cities,
        }
    }

    pub fn contains(&self, location: &Location) -> bool {
        self.code == location.code
            && self.coordinates
                == if self.cities {
                    location.point.map(coordinates)
                } else {
                    None
                }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DestinationKey {
    code: Box<str>,
    coordinates: (u64, u64),
    estimated: bool,
}

pub struct Located<'a> {
    pub observation: &'a Observation,
    pub location: Option<&'a Location>,
}

#[derive(Debug)]
pub struct Group {
    pub place: Place,
    pub point: Point,
    pub estimated: bool,
    pub observations: Vec<ObservationId>,
    pub active: usize,
    pub receiving: usize,
    pub sending: usize,
    pub unknown: usize,
    pub rates: Option<Rates>,
    pub introduced: Duration,
    pub regions: Vec<Region>,
    location: Location,
}

impl Group {
    fn new(location: &Location, point: Point, cities: bool) -> Self {
        Self {
            place: Place::new(location, cities),
            point,
            estimated: !cities,
            observations: Vec::new(),
            active: 0,
            receiving: 0,
            sending: 0,
            unknown: 0,
            rates: Some(Rates {
                received: 0.0,
                sent: 0.0,
            }),
            introduced: Duration::ZERO,
            regions: Vec::new(),
            location: location.clone(),
        }
    }

    fn add(&mut self, observation: &Observation, location: &Location) {
        self.observations.push(observation.id);
        self.estimated |= location.point.is_none();
        self.introduced = self.introduced.max(observation.introduced);
        match observation.rates {
            Some(rates) if rates.received > rates.sent => self.receiving += 1,
            Some(rates) if rates.sent > rates.received => self.sending += 1,
            _ => self.unknown += 1,
        }
        if observation
            .rates
            .is_some_and(|rates| rates.received + rates.sent > 0.0)
        {
            self.active += 1;
        }
        self.rates = self
            .rates
            .zip(observation.rates)
            .map(|(total, rates)| Rates {
                received: total.received + rates.received,
                sent: total.sent + rates.sent,
            });
    }

    pub fn name(&self) -> String {
        if !self.place.cities {
            return self.location.country.to_string();
        }
        match (&self.location.city, self.location.point) {
            (Some(city), _) => format!("{city}, {}", self.location.country),
            (_, Some(point)) => format!(
                "{:.3}, {:.3} · {}",
                point.latitude, point.longitude, self.location.country
            ),
            _ => self.location.country.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Destination {
    pub key: DestinationKey,
    pub group: usize,
    pub point: Point,
    pub share: f64,
    pub area: Option<Box<str>>,
    pub rank: usize,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub name: Box<str>,
    pub point: Point,
    population: f64,
}

#[derive(Default, Debug)]
pub struct Model {
    pub countries: HashMap<Box<str>, usize>,
    pub groups: Vec<Group>,
    pub destinations: Vec<Destination>,
    pub unlocated: usize,
}

impl Model {
    /// Worker-side arrangement uses fitted scale only. Zoom and pan consume the
    /// resulting destinations without redistributing the underlying observations.
    pub fn arrange<'a>(
        world: &World,
        observations: impl IntoIterator<Item = Located<'a>>,
        cities: bool,
        meridian: f32,
        scale: f32,
    ) -> Self {
        let countries: HashMap<_, _> = world
            .countries
            .iter()
            .map(|country| (country.code.as_ref(), country))
            .collect();
        let mut model = Self::default();
        let mut locations = HashMap::new();
        let mut country_order = Vec::new();
        let mut location_order = Vec::new();
        for Located {
            observation,
            location,
        } in observations
        {
            let Some(location) = location else {
                model.unlocated += 1;
                continue;
            };
            let country = countries.get(location.code.as_ref());
            let Some(point) = location
                .point
                .or_else(|| country.map(|country| country.anchor))
            else {
                model.unlocated += 1;
                continue;
            };
            let group = match model.countries.get(location.code.as_ref()) {
                Some(&group) => group,
                None => {
                    let group = model.groups.len();
                    model.groups.push(Group::new(
                        location,
                        country.map_or(point, |country| country.anchor),
                        false,
                    ));
                    model.groups[group].estimated = !cities;
                    model.countries.insert(location.code.clone(), group);
                    country_order.push(group);
                    group
                }
            };
            model.groups[group].add(observation, location);
            if !cities {
                continue;
            }
            let key = (location.code.as_ref(), location.point.map(coordinates));
            let group = *locations.entry(key).or_insert_with(|| {
                let group = model.groups.len();
                model.groups.push(Group::new(location, point, true));
                location_order.push(group);
                group
            });
            model.groups[group].add(observation, location);
        }
        for index in if cities {
            location_order
        } else {
            country_order
        } {
            let group = &mut model.groups[index];
            let country = countries.get(group.place.code.as_ref());
            if !group.estimated || country.is_none_or(|country| country.areas.is_empty()) {
                model
                    .destinations
                    .push(destination(index, group, group.point, 1.0, None, 0));
                continue;
            }
            group.regions = regions(&country.unwrap().areas, meridian, scale);
            let used = &group.regions[..reach(&group.regions, group.active.max(1))];
            let total: f64 = used.iter().map(|region| region.population).sum();
            for (rank, region) in used.iter().enumerate() {
                model.destinations.push(destination(
                    index,
                    group,
                    region.point,
                    region.population / total,
                    Some(region.name.clone()),
                    rank,
                ));
            }
        }
        model
    }

    pub fn marks(&self, selected: &HashSet<ObservationId>) -> HashMap<Place, usize> {
        let mut marks = HashMap::new();
        for destination in &self.destinations {
            let group = &self.groups[destination.group];
            if marks.contains_key(&group.place) {
                continue;
            }
            let count = group
                .observations
                .iter()
                .filter(|id| selected.contains(id))
                .count();
            if count > 0 {
                marks.insert(
                    group.place.clone(),
                    if group.regions.is_empty() {
                        1
                    } else {
                        reach(&group.regions, count)
                    },
                );
            }
        }
        marks
    }

    pub fn describe(&self, destination: &Destination) -> String {
        let group = &self.groups[destination.group];
        let rates = group.rates.map_or_else(
            || "Traffic unavailable".to_string(),
            |rates| format!("↓ {:.0} B/s · ↑ {:.0} B/s", rates.received, rates.sent),
        );
        let precision = destination.area.as_ref().map_or_else(
            || {
                if group.estimated {
                    "Country estimate".to_string()
                } else {
                    "Approximate location".to_string()
                }
            },
            |area| {
                format!(
                    "Estimated near {area} · {:.0}% of grouped traffic",
                    destination.share * 100.0
                )
            },
        );
        format!(
            "{} · {} connections · {} active · {rates} · {precision}",
            group.name(),
            group.observations.len(),
            group.active
        )
    }
}

fn destination(
    index: usize,
    group: &Group,
    point: Point,
    share: f64,
    area: Option<Box<str>>,
    rank: usize,
) -> Destination {
    Destination {
        key: DestinationKey {
            code: group.place.code.clone(),
            coordinates: coordinates(point),
            estimated: group.estimated,
        },
        group: index,
        point,
        share,
        area,
        rank,
    }
}

fn regions(areas: &[Area], meridian: f32, scale: f32) -> Vec<Region> {
    let mut regions: Vec<Region> = Vec::new();
    let mut positions: Vec<Vector2> = Vec::new();
    for area in areas {
        let position = Vector2::new(
            wrap((area.point.longitude - f64::from(meridian)) as f32, 0.0),
            -area.point.latitude as f32,
        ) * scale;
        if let Some(index) = positions
            .iter()
            .position(|&other| (other - position).length_squared().sqrt() < 25.0)
        {
            regions[index].population += area.population;
        } else {
            regions.push(Region {
                name: area.name.clone(),
                point: area.point,
                population: area.population,
            });
            positions.push(position);
        }
    }
    regions.sort_by(|a, b| b.population.total_cmp(&a.population));
    regions
}

// A region first receives a connection once each more populous region has
// floor(Pj / Pi) connections. This is the source's D'Hondt placement rule.
fn reach(regions: &[Region], connections: usize) -> usize {
    let mut reached = 0;
    while reached < regions.len() {
        let mut first = 1_u64;
        for region in regions.iter().take(reached) {
            if first > connections as u64 {
                break;
            }
            first += (region.population / regions[reached].population).floor() as u64;
        }
        if first > connections as u64 {
            break;
        }
        reached += 1;
    }
    reached
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::{Bytes, Endpoint, Life, ProcessId, TcpState, Tuple};

    #[test]
    fn marker_kinds_count_each_observation_once_including_unknown_activity() {
        let point = Point {
            latitude: 10.0,
            longitude: 20.0,
        };
        let location = Location {
            code: "US".into(),
            country: "United States".into(),
            city: None,
            point: Some(point),
        };
        let mut group = Group::new(&location, point, true);
        for (index, rates) in [
            Some(Rates {
                received: 4.0,
                sent: 1.0,
            }),
            Some(Rates {
                received: 1.0,
                sent: 4.0,
            }),
            Some(Rates {
                received: 2.0,
                sent: 2.0,
            }),
            Some(Rates {
                received: 0.0,
                sent: 0.0,
            }),
            None,
        ]
        .into_iter()
        .enumerate()
        {
            group.add(
                &Observation {
                    id: ObservationId(index as u64),
                    tuple: Tuple {
                        process: ProcessId {
                            pid: 1,
                            generation: 1,
                        },
                        local: Endpoint::new("127.0.0.1:1".parse().unwrap()),
                        remote: Endpoint::new("8.8.8.8:443".parse().unwrap()),
                    },
                    life: Life::Tcp(TcpState::Established),
                    introduced: Duration::ZERO,
                    bytes: Bytes::default(),
                    rates,
                },
                &location,
            );
        }
        assert_eq!((group.receiving, group.sending, group.unknown), (1, 1, 3));
        assert_eq!(
            group.receiving + group.sending + group.unknown,
            group.observations.len()
        );
        assert_eq!(group.active, 3);
    }
}
