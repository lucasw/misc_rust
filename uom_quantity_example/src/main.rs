use typenum::{N1, Z0};
use uom::{
    Kind,
    si::{
        ISQ, Quantity, SI,
        f64::{Length, V},
        length::meter,
    },
};

type InverseMeter = Quantity<ISQ<N1, Z0, Z0, Z0, Z0, Z0, Z0, dyn Kind>, SI<V>, V>;

// TODO(lucasw) curvature has a special meaing that shouldn't be mixed up with some
// other application of inverse meter, need to wrap it in a struct to enforce that?
#[derive(Debug)]
struct TurnRadius(Length);
#[derive(Debug)]
struct Curvature(InverseMeter);

impl TurnRadius {
    fn new<T>(value: V) -> Self
    where T: uom::si::length::Unit + uom::Conversion<V, T = V>
    {
        TurnRadius(Length::new::<T>(value))
    }

    pub fn from_curvature(curvature: &Curvature) -> Self {
        Self(1.0 / curvature.0)
    }

    pub fn to_curvature(&self) -> Curvature {
        Curvature(1.0 / self.0)
    }
}

fn main() {
    // not using wrapper types
    let turn_radius0 = Length::new::<meter>(4.0);
    let curvature0: InverseMeter = 1.0 / turn_radius0;
    println!("turn radius {turn_radius0:?} -> curvature {curvature0:?}");

    let turn_radius1 = TurnRadius::new::<meter>(10.0);
    // let curvature: Curvature = 1.0 / turn_radius;
    // these fail to compile as desired
    // turn_radius1 = turn_radius0;
    // println!("turn radius {turn_radius1:?} ->  {}", turn_radius0 == turn_radius1);

    // but can't yet 1.0 / turn_radius without Div impl above
    let curvature1: Curvature = turn_radius1.to_curvature();
    let turn_radius1b = TurnRadius::from_curvature(&curvature1);
    println!("turn radius {turn_radius1:?} -> curvature {curvature1:?} -> {turn_radius1b:?}");
}
