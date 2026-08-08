#![allow(non_camel_case_types)]

use crate::*;
use crate::poly_containers::*;


pub trait UfoStereotype   {

    fn name<'a>(&'a self) -> &'a str;
    // fn name_mut(&mut self) -> &mut &'a str;
    // fn set_name(&mut self, value: String);


}

impl UfoStereotype for crate::UfoStereotype {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}
impl UfoStereotype for crate::Kind {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}
impl UfoStereotype for crate::SubKind {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}
impl UfoStereotype for crate::Role {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}
impl UfoStereotype for crate::Relator {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}
impl UfoStereotype for crate::Mode {
        fn name<'a>(&'a self) -> &'a str {
        return &self.name[..];
    }
}

impl UfoStereotype for crate::UfoStereotypeOrSubtype {
        fn name<'a>(&'a self) -> &'a str {
        match self {
                UfoStereotypeOrSubtype::Kind(val) => val.name(),
                UfoStereotypeOrSubtype::SubKind(val) => val.name(),
                UfoStereotypeOrSubtype::Role(val) => val.name(),
                UfoStereotypeOrSubtype::Relator(val) => val.name(),
                UfoStereotypeOrSubtype::Mode(val) => val.name(),

        }
    }
}

pub trait Kind : UfoStereotype   {


}

impl Kind for crate::Kind {
}


pub trait SubKind : UfoStereotype   {

    fn parent<'a>(&'a self) -> &'a str;
    // fn parent_mut(&mut self) -> &mut &'a str;
    // fn set_parent(&mut self, value: String);


}

impl SubKind for crate::SubKind {
        fn parent<'a>(&'a self) -> &'a str {
        return &self.parent[..];
    }
}


pub trait Role : UfoStereotype   {


}

impl Role for crate::Role {
}


pub trait Relator : UfoStereotype   {


}

impl Relator for crate::Relator {
}


pub trait Mode : UfoStereotype   {


}

impl Mode for crate::Mode {
}
