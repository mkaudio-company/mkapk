use gui_core::{BLACK, Color, CommandList, PaintCommand, Pointf, Rectf, Sizef, WHITE};

const COUNT: usize = 10_000;

#[test]
fn command_list_retains_capacity_after_clear_and_repush() {
    let mut list = CommandList::with_capacity(COUNT);

    for _ in 0..COUNT {
        list.push(PaintCommand::Clear { color: BLACK });
    }
    assert_eq!(list.len(), COUNT);

    let capacity_before = list.capacity();
    list.clear();
    assert!(list.is_empty());
    assert_eq!(list.capacity(), capacity_before);

    for _ in 0..COUNT {
        list.push(PaintCommand::FillRect {
            rect: Rectf::new(Pointf::new(0.0, 0.0), Sizef::new(1.0, 1.0)),
            color: WHITE,
        });
    }
    assert_eq!(list.len(), COUNT);
    assert_eq!(list.capacity(), capacity_before);
}

#[test]
fn command_list_iteration_count_matches_len() {
    let mut list = CommandList::with_capacity(COUNT);

    for i in 0..COUNT {
        list.push(PaintCommand::Clear {
            color: Color::new((i % 256) as u8, 0, 0, 255),
        });
    }

    let mut iterated = 0;
    for cmd in &list {
        match cmd {
            PaintCommand::Clear { .. } => iterated += 1,
            _ => panic!("unexpected command variant"),
        }
    }
    assert_eq!(iterated, list.len());
    assert_eq!(iterated, COUNT);
}
