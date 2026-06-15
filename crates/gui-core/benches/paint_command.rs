use criterion::{Criterion, black_box, criterion_group, criterion_main};

use gui_core::{Color, CommandList, PaintCommand, Pointf, Rectf, Sizef};

const COUNT: usize = 10_000;

fn bench_command_list_reuse(c: &mut Criterion) {
    let mut list = CommandList::with_capacity(COUNT);

    c.bench_function("command_list/fill_rect_reuse", |b| {
        b.iter(|| {
            list.clear();

            for i in 0..COUNT {
                let i_f = i as f32;
                let cmd = PaintCommand::FillRect {
                    rect: Rectf::new(
                        Pointf::new(i_f, i_f * 0.5),
                        Sizef::new(10.0 + i_f % 100.0, 10.0 + i_f % 100.0),
                    ),
                    color: Color::new(
                        (i % 256) as u8,
                        ((i / 256) % 256) as u8,
                        ((i / 65_536) % 256) as u8,
                        255,
                    ),
                };
                list.push(cmd);
            }

            black_box(&list);
        });

        black_box(list.len());
    });
}

criterion_group!(command_list, bench_command_list_reuse);
criterion_main!(command_list);
