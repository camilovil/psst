use druid::{widget::prelude::*, Point, WidgetPod};

/// Lays out `main` with `side` docked to its right. The side panel is shown
/// only while `is_visible` holds and the window is wide enough to keep
/// `min_main_width` for the main content, so narrow windows lose nothing.
pub struct SidePanel<T> {
    main: WidgetPod<T, Box<dyn Widget<T>>>,
    side: WidgetPod<T, Box<dyn Widget<T>>>,
    is_visible: Box<dyn Fn(&T) -> bool>,
    side_width: f64,
    min_main_width: f64,
    gap: f64,
    side_shown: bool,
}

impl<T: Data> SidePanel<T> {
    pub fn new(
        main: impl Widget<T> + 'static,
        side: impl Widget<T> + 'static,
        is_visible: impl Fn(&T) -> bool + 'static,
    ) -> Self {
        Self {
            main: WidgetPod::new(main).boxed(),
            side: WidgetPod::new(side).boxed(),
            is_visible: Box::new(is_visible),
            side_width: 0.0,
            min_main_width: 0.0,
            gap: 0.0,
            side_shown: false,
        }
    }

    pub fn side_width(mut self, width: f64) -> Self {
        self.side_width = width;
        self
    }

    pub fn min_main_width(mut self, width: f64) -> Self {
        self.min_main_width = width;
        self
    }

    pub fn gap(mut self, gap: f64) -> Self {
        self.gap = gap;
        self
    }
}

impl<T: Data> Widget<T> for SidePanel<T> {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        self.main.event(ctx, event, data, env);
        // A hidden side panel sits outside our bounds, so it gets no pointer
        // input, but it still needs commands (e.g. images finishing loading).
        self.side.event(ctx, event, data, env);
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        self.main.lifecycle(ctx, event, data, env);
        self.side.lifecycle(ctx, event, data, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
        if (self.is_visible)(old_data) != (self.is_visible)(data) {
            ctx.request_layout();
        }
        self.main.update(ctx, data, env);
        self.side.update(ctx, data, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        let size = bc.max();
        self.side_shown = (self.is_visible)(data)
            && size.width - self.side_width - self.gap >= self.min_main_width;

        let main_width = if self.side_shown {
            size.width - self.side_width - self.gap
        } else {
            size.width
        };
        let main_size = Size::new(main_width, size.height);
        self.main.layout(ctx, &BoxConstraints::tight(main_size), data, env);
        self.main.set_origin(ctx, Point::ORIGIN);

        // The side panel is always laid out, so it is ready when it appears.
        let side_size = Size::new(self.side_width, size.height);
        self.side.layout(ctx, &BoxConstraints::tight(side_size), data, env);
        let side_x = if self.side_shown {
            main_width + self.gap
        } else {
            size.width + self.gap
        };
        self.side.set_origin(ctx, Point::new(side_x, 0.0));

        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        self.main.paint(ctx, data, env);
        if self.side_shown {
            self.side.paint(ctx, data, env);
        }
    }
}
