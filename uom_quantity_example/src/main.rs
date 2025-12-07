use typenum::{N1, Z0};
use uom::{
    Kind,
    num_traits::Zero,
    quantity,
    si::{
        ISQ, Quantity, SI,
        f64::{Length, V},
        length::meter,
    },
};

type InverseLength = Quantity<ISQ<N1, Z0, Z0, Z0, Z0, Z0, Z0, dyn Kind>, SI<V>, V>;

/*
quantity! {
    /// Inverse meter (base unit radian per second, s⁻¹).
    quantity: InverseLength; "inverse meter";
    /// Dimension of angular velocity, T⁻¹ (base unit radian per second, s⁻¹).
    dimension: ISQ<
        N1,     // length
        Z0,     // mass
        Z0,     // time
        Z0,     // electric current
        Z0,     // thermodynamic temperature
        Z0,     // amount of substance
        Z0>;    // luminous intensity
    kind: dyn uom::si::marker::AngleKind;
    units {
        /// Derived unit of angular velocity.
        @per_meter: 1.0_E0; "/m", "meter", "per meter";
    }
}
*/

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
struct Curvature(InverseLength);

impl Curvature {
    fn new<T>(value: V) -> Self
    where
        T: uom::si::length::Unit + uom::Conversion<V, T = V>,
    {
        // TODO(lucasw) need inverse_meter to set directly
        TurnRadius(Length::new::<T>(1.0 / value)).to_curvature()
    }

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
    let curvature0: InverseLength = 1.0 / turn_radius0;
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

    for curvature_value in [10.0, -10.0, 0.0, f64::INFINITY, f64::NEG_INFINITY] {
        let curvature: Curvature = Curvature::new::<meter>(curvature_value);
        println!("{curvature_value} -> curvature {curvature:?}");
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
