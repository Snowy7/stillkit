use super::*;
use stillkit::el::paint;
/// Owned area series; paint resolves its colour from the current theme.
pub struct ChartSeries {
    pub label: Arc<str>,
    pub values: Vec<f64>,
    pub color: Paint,
}
impl ChartSeries {
    pub fn new(label: impl Into<Arc<str>>, values: Vec<f64>, color: impl Into<Paint>) -> Self {
        Self {
            label: label.into(),
            values,
            color: color.into(),
        }
    }
}
/// The old kit's smooth area plot, hosted by an el paint node.
/// This uses public chart painting because el has no arbitrary path primitive.
pub struct Chart {
    label: Arc<str>,
    series: Vec<ChartSeries>,
    labels: [Arc<str>; 3],
}
impl Chart {
    pub fn new(label: impl Into<Arc<str>>, series: Vec<ChartSeries>) -> Self {
        Self {
            label: label.into(),
            series,
            labels: ["Start".into(), "Middle".into(), "Now".into()],
        }
    }
    pub fn x_labels(mut self, labels: [&str; 3]) -> Self {
        self.labels = labels.map(Into::into);
        self
    }
    pub fn build<S: 'static>(self) -> El<S> {
        let summary = self
            .series
            .iter()
            .map(|s| format!("{}: {}", s.label, s.values.last().unwrap_or(&0.)))
            .collect::<Vec<_>>()
            .join(", ");
        paint(move |f, rect, _| {
            let series = self
                .series
                .iter()
                .map(|s| stillkit::chart::Series {
                    color: s.color.resolve(f.theme),
                    values: &s.values,
                })
                .collect::<Vec<_>>();
            stillkit::chart::area_chart(
                f,
                rect,
                &series,
                [&self.labels[0], &self.labels[1], &self.labels[2]],
                |v| format!("{v:.0}"),
            );
        })
        .h(180.)
        .role(Role::Image)
        .label(self.label)
        .value(summary)
    }
}
/// The old quota bar with pace, hatching, monogram and reset countdown.
pub struct QuotaBar {
    label: Arc<str>,
    left: f32,
    pace: Option<f32>,
    badge: Option<Arc<str>>,
    reset: Option<Arc<str>>,
}
impl QuotaBar {
    pub fn new(label: impl Into<Arc<str>>, left: f32) -> Self {
        Self {
            label: label.into(),
            left: unit(left),
            pace: None,
            badge: None,
            reset: None,
        }
    }
    pub fn pace(mut self, pace: f32) -> Self {
        self.pace = Some(unit(pace));
        self
    }
    pub fn badge(mut self, badge: impl Into<Arc<str>>) -> Self {
        self.badge = Some(badge.into());
        self
    }
    pub fn reset(mut self, reset: impl Into<Arc<str>>) -> Self {
        self.reset = Some(reset.into());
        self
    }
    pub fn build<S: 'static>(self) -> El<S> {
        let value = format!("{}% left", (self.left * 100.).round());
        let left = self.left;
        let pace = self.pace;
        let mut content = row().w_full().pad_x(8.).gap(6.).align(Align::Center).h(32.);
        if let Some(badge) = self.badge {
            content = content.child(Avatar::new("Account", badge).size(20.).build());
        }
        content = content.child(
            text(value.clone())
                .color(LABEL)
                .font(TextRole::Caption)
                .grow(1.),
        );
        if let Some(reset) = self.reset {
            content = content.child(Badge::new(reset).build());
        }
        paint(move |f, rect, _| {
            let bar = stillkit::chart::LimitBar {
                left,
                pace,
                fill: f.theme.color.accent_wash,
                badge: None,
                label: None,
                reset: None,
            };
            stillkit::chart::limit_bar(f, rect, &bar);
        })
        .h(32.)
        .role(Role::Meter)
        .label(self.label)
        .range(0., 1., 0.01, left.into())
        .value(value)
        .child(content)
    }
}
