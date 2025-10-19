// tests/core_tests.rs
use atomas_core::{
    elements::{Element, Id, ElementType, SpecialAtom},
    ring::{CircularList, AdjMatrix},
};

#[test]
fn test_adjmatrix() {
    let element = Element {
        id: Id::Single('H'),
        element_type: ElementType::Periodic(1),
        name: "Hydrogen",
        rgb: (255, 255, 255),
    };
    
    let special = Element {
        id: Id::Single('+'),
        element_type: ElementType::Special(SpecialAtom::Plus),
        name: "Plus",
        rgb: (255, 255, 255),
    };

    let mut ring = CircularList::new();
    ring.insert(element, 0);
    ring.insert(special, 1);

    let mut adjmatrix = AdjMatrix::new(12);
    adjmatrix.update_from_ring(&ring, &element);
    println!("{}", adjmatrix);
}
