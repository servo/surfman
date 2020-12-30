//! Your euclid replacement, with any kind of shape you would ever need.
#[macro_use]
mod macros;

mod angle;
pub mod approxeq;
pub mod approxord;
mod box2d;
mod box3d;
mod homogen;
mod length;
pub mod num;
mod point;
mod rect;
mod rigid;
mod rotation;
mod scale;
mod side_offsets;
mod size;
mod transform2d;
mod transform3d;
mod translation;
mod trig;
pub mod vector;

pub use crate::geom::angle::Angle;
pub use crate::geom::box2d::Box2D;
pub use crate::geom::homogen::HomogeneousVector;
pub use crate::geom::length::Length;
pub use crate::geom::point::{point2, point3, Point2D, Point3D};
pub use crate::geom::scale::Scale;
pub use crate::geom::transform2d::Transform2D;
pub use crate::geom::transform3d::Transform3D;
pub use crate::geom::vector::{bvec2, bvec3, BoolVector2D, BoolVector3D};
pub use crate::geom::vector::{vec2, vec3, Vector2D, Vector3D};

pub use crate::geom::box3d::{box3d, Box3D};
pub use crate::geom::rect::{rect, Rect};
pub use crate::geom::rigid::RigidTransform3D;
pub use crate::geom::rotation::{Rotation2D, Rotation3D};
pub use crate::geom::side_offsets::SideOffsets2D;
pub use crate::geom::size::{size2, size3, Size2D, Size3D};
pub use crate::geom::translation::{Translation2D, Translation3D};
pub use crate::geom::trig::Trig;

/// The default unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnknownUnit;

pub mod default {
    //! A set of aliases for all types, tagged with the default unknown unit.

    use super::UnknownUnit;
    /// Length with no unit.
    pub type Length<T> = super::Length<T, UnknownUnit>;
    /// Point with no unit.
    pub type Point2D<T> = super::Point2D<T, UnknownUnit>;
    /// 3DPoint with no unit.
    pub type Point3D<T> = super::Point3D<T, UnknownUnit>;
    /// Vector with no unit.
    pub type Vector2D<T> = super::Vector2D<T, UnknownUnit>;
    /// 3DVector with no unit.
    pub type Vector3D<T> = super::Vector3D<T, UnknownUnit>;
    /// Homogeneous Vector with no unit.
    pub type HomogeneousVector<T> = super::HomogeneousVector<T, UnknownUnit>;
    /// Size with no unit.
    pub type Size2D<T> = super::Size2D<T, UnknownUnit>;
    /// 3DSize with no unit.
    pub type Size3D<T> = super::Size3D<T, UnknownUnit>;
    /// Rect with no unit.
    pub type Rect<T> = super::Rect<T, UnknownUnit>;
    /// Box with no unit.
    pub type Box2D<T> = super::Box2D<T, UnknownUnit>;
    /// 3D Box with no unit.
    pub type Box3D<T> = super::Box3D<T, UnknownUnit>;
    /// Side offsets with no unit.
    pub type SideOffsets2D<T> = super::SideOffsets2D<T, UnknownUnit>;
    /// Transform with no unit.
    pub type Transform2D<T> = super::Transform2D<T, UnknownUnit, UnknownUnit>;
    /// 3D Transform with no unit.
    pub type Transform3D<T> = super::Transform3D<T, UnknownUnit, UnknownUnit>;
    /// Rotation with no unit.
    pub type Rotation2D<T> = super::Rotation2D<T, UnknownUnit, UnknownUnit>;
    /// 3D Rotation with no unit.
    pub type Rotation3D<T> = super::Rotation3D<T, UnknownUnit, UnknownUnit>;
    /// Translation with no unit.
    pub type Translation2D<T> = super::Translation2D<T, UnknownUnit, UnknownUnit>;
    /// 3DTranslation with no unit.
    pub type Translation3D<T> = super::Translation3D<T, UnknownUnit, UnknownUnit>;
    /// Scale with no unit.
    pub type Scale<T> = super::Scale<T, UnknownUnit, UnknownUnit>;
    /// Rigid 3D Transformation with no unit.
    pub type RigidTransform3D<T> = super::RigidTransform3D<T, UnknownUnit, UnknownUnit>;
}
