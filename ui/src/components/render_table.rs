use geom::Polygon;
use widgetry::{Color, ControlState, EventCtx, GeomBatch, Widget};

// TODO This is copied from widgetry to update it; move it back there
pub fn render_table(
    ctx: &mut EventCtx,
    headers: Vec<Widget>,
    rows: Vec<(String, Vec<GeomBatch>)>,
    total_width: f64,
    min_extra_margin: f64,
) -> Widget {
    let mut width_per_col: Vec<f64> = headers.iter().map(|w| w.get_width_for_forcing()).collect();
    for (_, row) in &rows {
        for (col, width) in row.iter().zip(width_per_col.iter_mut()) {
            *width = width.max(col.get_dims().width);
        }
    }

    let actual_total_width = width_per_col.clone().into_iter().sum::<f64>();
    let extra_margin = ((total_width - actual_total_width) / (width_per_col.len() - 1) as f64)
        .max(min_extra_margin);
    //println!("total_width wanted {total_width}, actual {actual_total_width}, extra margin {extra_margin}");

    let mut col = vec![Widget::custom_row(
        headers
            .into_iter()
            .enumerate()
            .map(|(idx, w)| {
                let margin = extra_margin + width_per_col[idx] - w.get_width_for_forcing();
                //println!("margin for col {idx} is {margin}. {extra_margin} + {} - {}", width_per_col[idx], w.get_width_for_forcing());
                if idx == width_per_col.len() - 1 {
                    w.margin_right((margin - extra_margin) as usize)
                } else {
                    w.margin_right(margin as usize)
                }
            })
            .collect(),
    )];

    // TODO Maybe can do this now simpler with to_geom
    for (label, row) in rows {
        let mut batch = GeomBatch::new();
        batch.autocrop_dims = false;
        let mut x1 = 0.0;
        for (col, width) in row.into_iter().zip(width_per_col.iter()) {
            batch.append(col.translate(x1, 0.0));
            x1 += *width + extra_margin;
        }

        // TODO What if we exceed this?
        let rect = Polygon::rectangle(total_width, batch.get_dims().height);
        let mut hovered = GeomBatch::new();
        hovered.push(Color::hex("#7C7C7C"), rect.clone());
        hovered.append(batch.clone());

        col.push(
            ctx.style()
                .btn_plain
                .btn()
                .custom_batch(batch, ControlState::Default)
                .custom_batch(hovered, ControlState::Hovered)
                .no_tooltip()
                .build_widget(ctx, &label),
        );
    }

    Widget::custom_col(col)
}
