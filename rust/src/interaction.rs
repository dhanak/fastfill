use crate::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchEvent {
    Pressed(u64, Point),
    Moved(u64, Point),
    Released(u64, Point),
    Lost(u64, Point),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureDelta {
    pub pan: Point,
    pub zoom_factor: f32,
    pub zoom_anchor: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TouchPoint {
    id: u64,
    position: Point,
}

#[derive(Debug, Default)]
pub struct TouchNavigation {
    touches: Vec<TouchPoint>,
}

impl TouchNavigation {
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.touches.is_empty()
    }

    pub fn handle(
        &mut self,
        event: TouchEvent,
        navigation_enabled: bool,
    ) -> Option<GestureDelta> {
        if !navigation_enabled {
            self.touches.clear();
            return None;
        }

        match event {
            TouchEvent::Pressed(id, position) => {
                if valid(position)
                    && self.touches.len() < 2
                    && self.index_of(id).is_none()
                {
                    self.touches.push(TouchPoint { id, position });
                }
                None
            }
            TouchEvent::Moved(id, position) => self.move_to(id, position),
            TouchEvent::Released(id, _) | TouchEvent::Lost(id, _) => {
                self.touches.retain(|touch| touch.id != id);
                None
            }
        }
    }

    fn move_to(&mut self, id: u64, position: Point) -> Option<GestureDelta> {
        if !valid(position) {
            return None;
        }
        let index = self.index_of(id)?;

        if self.touches.len() == 1 {
            let previous = self.touches[index].position;
            self.touches[index].position = position;
            return changed(GestureDelta {
                pan: subtract(position, previous),
                zoom_factor: 1.0,
                zoom_anchor: previous,
            });
        }

        let previous_center =
            center(self.touches[0].position, self.touches[1].position);
        let previous_distance =
            distance(self.touches[0].position, self.touches[1].position);
        self.touches[index].position = position;
        let current_center =
            center(self.touches[0].position, self.touches[1].position);
        let current_distance =
            distance(self.touches[0].position, self.touches[1].position);
        let zoom_factor = if previous_distance > f32::EPSILON
            && current_distance > f32::EPSILON
        {
            current_distance / previous_distance
        } else {
            1.0
        };

        changed(GestureDelta {
            pan: subtract(current_center, previous_center),
            zoom_factor,
            zoom_anchor: previous_center,
        })
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.touches.iter().position(|touch| touch.id == id)
    }
}

fn valid(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn changed(delta: GestureDelta) -> Option<GestureDelta> {
    (delta.pan.x != 0.0
        || delta.pan.y != 0.0
        || (delta.zoom_factor - 1.0).abs() > f32::EPSILON)
        .then_some(delta)
}

fn center(first: Point, second: Point) -> Point {
    Point::new(
        f32::midpoint(first.x, second.x),
        f32::midpoint(first.y, second.y),
    )
}

fn subtract(first: Point, second: Point) -> Point {
    Point::new(first.x - second.x, first.y - second.y)
}

fn distance(first: Point, second: Point) -> f32 {
    let delta = subtract(first, second);
    delta.x.hypot(delta.y)
}
