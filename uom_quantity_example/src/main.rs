use typenum::{N1, Z0};
use uom::{
    Kind,
    num_traits::Zero,
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

impl TurnRadius {
    fn new<T>(value: V) -> Self
    where
        T: uom::si::length::Unit + uom::Conversion<V, T = V>,
    {
        TurnRadius(Length::new::<T>(value))
    }

    pub fn from_curvature(curvature: &Curvature) -> Self {
        Self(1.0 / curvature.0)
    }

    pub fn to_curvature(&self) -> Curvature {
        Curvature(1.0 / self.0)
    }

    pub fn zero() -> Self {
        Self(Length::zero())
    }

    pub fn inf<T>() -> Self
    where
        T: uom::si::length::Unit + uom::Conversion<V, T = V>,
    {
        Self(Length::new::<T>(V::INFINITY))
    }
}

#[derive(Debug)]
struct Curvature(InverseMeter);

impl Curvature {
    pub fn zero<T>() -> Self
    where
        T: uom::si::length::Unit + uom::Conversion<V, T = V>,
    {
        TurnRadius::inf::<T>().to_curvature()
    }
}

fn main() {
    // not using wrapper types
    let turn_radius0 = Length::new::<meter>(4.0);
    let curvature0: InverseMeter = 1.0 / turn_radius0;
    println!("turn radius {turn_radius0:?} -> curvature {curvature0:?}");

    let _turn_radius1 = TurnRadius::new::<meter>(10.0);
    // let curvature: Curvature = 1.0 / turn_radius;
    // these fail to compile as desired
    // turn_radius1 = turn_radius0;
    // println!("turn radius {turn_radius1:?} ->  {}", turn_radius0 == turn_radius1);

    for radius in [10.0, -10.0, 0.0, f64::INFINITY, f64::NEG_INFINITY] {
        let turn_radius2 = TurnRadius::new::<meter>(radius);
        let curvature2: Curvature = turn_radius2.to_curvature();
        let turn_radius2b = TurnRadius::from_curvature(&curvature2);
        println!("turn radius {turn_radius2:?} -> curvature {curvature2:?} -> {turn_radius2b:?}");
    }

    let turn_radius = TurnRadius::zero();
    println!("turn_radius {turn_radius:?}");
    // TODO(lucasw) it seems like meter shouldn't be required
    let turn_radius = TurnRadius::inf::<meter>();
    println!("turn_radius {turn_radius:?}");

    let curvature = Curvature::zero::<meter>();
    println!("curvature {curvature:?}");
    // TODO(lucasw) want to have inverse_meter
    // let curvature = Curvature::new::<inverse_meter>(0.0);
}
