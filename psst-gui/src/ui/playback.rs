use std::time::Duration;

use druid::{
    kurbo::{Affine, BezPath},
    widget::{
        CrossAxisAlignment, Either, Flex, Label, LineBreaking, MainAxisAlignment, SizedBox,
        Spinner, ViewSwitcher,
    },
    BoxConstraints, Cursor, Data, Env, Event, EventCtx, LayoutCtx, LensExt, LifeCycle,
    LifeCycleCtx, MouseButton, PaintCtx, Rect, RenderContext, Size, UpdateCtx, Widget,
    WidgetExt,
};
use itertools::Itertools;

use crate::{
    cmd::{self, ADD_TO_QUEUE, SHOW_ARTWORK, TOGGLE_LYRICS},
    controller::PlaybackController,
    data::{
        AppState, AudioAnalysis, Episode, NowPlaying, Playable, PlayableMatcher, Playback,
        PlaybackOrigin, PlaybackState, QueueBehavior, ShowLink, Track,
    },
    widget::{
        icons::{self, SvgIcon},
        Empty, Maybe, MyWidgetExt, RemoteImage,
    },
};

use super::{episode, library, theme, track, utils};

pub fn panel_widget() -> impl Widget<AppState> {
    let item_info = Maybe::or_empty(playing_item_widget).lens(Playback::now_playing);
    let controls = Either::new(
        |playback: &Playback, _| playback.now_playing.is_some(),
        player_widget(),
        Empty,
    );
    let extras = Flex::row()
        .must_fill_main_axis(true)
        .main_axis_alignment(MainAxisAlignment::End)
        .with_child(
            small_button_widget(&icons::MUSIC_NOTE).on_left_click(|ctx, _, _, _| {
                ctx.submit_command(TOGGLE_LYRICS);
            }),
        )
        .with_child(super::volume_slider().fix_width(theme::grid(18.0)));

    // Three columns like Spotify: track info, centered controls, extras.
    Flex::row()
        .must_fill_main_axis(true)
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .with_flex_child(item_info.lens(AppState::playback), 3.0)
        .with_flex_child(controls.lens(AppState::playback), 4.0)
        .with_flex_child(extras, 3.0)
        .padding(theme::GRID)
        .fix_height(theme::grid(11.0))
        .controller(PlaybackController::new())
        .on_command(ADD_TO_QUEUE, |_, _, data: &mut AppState| {
            data.info_alert("Track added to queue.")
        })
}

fn playing_item_widget() -> impl Widget<NowPlaying> {
    let cover_art = cover_widget(theme::grid(7.0));

    let name = PlayableMatcher::new()
        .track(
            Label::raw()
                .with_line_break_mode(LineBreaking::Clip)
                .with_font(theme::UI_FONT_MEDIUM)
                .lens(Track::name.in_arc()),
        )
        .episode(
            Label::raw()
                .with_line_break_mode(LineBreaking::Clip)
                .with_font(theme::UI_FONT_MEDIUM)
                .lens(Episode::name.in_arc()),
        )
        .lens(NowPlaying::item);

    let detail = PlayableMatcher::new()
        .track(
            Label::raw()
                .with_line_break_mode(LineBreaking::Clip)
                .with_text_size(theme::TEXT_SIZE_SMALL)
                .lens(Track::lens_artist_name().in_arc()),
        )
        .episode(
            Label::raw()
                .with_line_break_mode(LineBreaking::Clip)
                .with_text_size(theme::TEXT_SIZE_SMALL)
                .lens(Episode::show.in_arc().then(ShowLink::name)),
        )
        .lens(NowPlaying::item);

    let origin = ViewSwitcher::new(
        |origin: &PlaybackOrigin, _| origin.clone(),
        |origin, _, _| {
            Flex::row()
                .cross_axis_alignment(CrossAxisAlignment::Center)
                .with_flex_child(
                    Label::dynamic(|origin: &PlaybackOrigin, _| origin.to_string())
                        .with_line_break_mode(LineBreaking::Clip)
                        .with_text_size(theme::TEXT_SIZE_SMALL),
                    1.0,
                )
                .with_spacer(theme::grid(0.25))
                .with_child(playback_origin_icon(origin).scale(theme::ICON_SIZE_SMALL))
                .boxed()
        },
    )
    .lens(NowPlaying::origin);

    Flex::row()
        .with_child(cover_art)
        .with_flex_child(
            Flex::row().with_spacer(theme::grid(2.0)).with_flex_child(
                Flex::column()
                    .cross_axis_alignment(CrossAxisAlignment::Start)
                    .with_child(name)
                    .with_spacer(2.0)
                    .with_child(detail)
                    .with_spacer(2.0)
                    .with_child(origin)
                    .on_click(|ctx, now_playing, _| {
                        ctx.submit_command(cmd::NAVIGATE.with(now_playing.origin.to_nav()));
                    })
                    .context_menu(|now_playing| match &now_playing.item {
                        Playable::Track(track) => {
                            track::track_menu(track, &now_playing.library, &now_playing.origin)
                        }
                        Playable::Episode(episode) => {
                            episode::episode_menu(episode, &now_playing.library)
                        }
                    }),
                1.0,
            ),
            1.0,
        )
        .with_child(ViewSwitcher::new(
            |now_playing: &NowPlaying, _| {
                now_playing.item.track().is_some() && now_playing.library.saved_tracks.is_resolved()
            },
            |selector, _data, _env| match selector {
                true => {
                    // View is only show if now_playing's track isn't none
                    ViewSwitcher::new(
                        |now_playing: &NowPlaying, _| {
                            now_playing
                                .library
                                .contains_track(now_playing.item.track().unwrap())
                        },
                        |selector: &bool, _, _| {
                            match selector {
                                true => &icons::CIRCLE_CHECK,
                                false => &icons::CIRCLE_PLUS,
                            }
                            .scale(theme::ICON_SIZE_SMALL)
                            .boxed()
                        },
                    )
                    .on_left_click(|ctx, _, now_playing, _| {
                        let track = now_playing.item.track().unwrap();
                        if now_playing.library.contains_track(track) {
                            ctx.submit_command(library::UNSAVE_TRACK.with(track.id))
                        } else {
                            ctx.submit_command(library::SAVE_TRACK.with(track.clone()))
                        }
                    })
                    .padding(theme::grid(1.0))
                    .boxed()
                }
                false => Box::new(Flex::column()),
            },
        ))
        .padding(theme::grid(1.0))
        .link()
}

fn cover_widget(size: f64) -> impl Widget<NowPlaying> {
    RemoteImage::new(utils::placeholder_widget(), move |np: &NowPlaying, _| {
        np.cover_image_url(size, size).map(|url| url.into())
    })
    .fix_size(size, size)
    .clip(Size::new(size, size).to_rounded_rect(theme::cover_radius(size)))
    .on_left_click(|ctx, _, _, _| {
        ctx.submit_command(SHOW_ARTWORK);
    })
}

fn playback_origin_icon(origin: &PlaybackOrigin) -> &'static SvgIcon {
    match origin {
        PlaybackOrigin::Home => &icons::HOME,
        PlaybackOrigin::Library => &icons::HEART,
        PlaybackOrigin::Album { .. } => &icons::ALBUM,
        PlaybackOrigin::Playlist { .. } => &icons::PLAYLIST,
        PlaybackOrigin::Show { .. } => &icons::PODCAST,
        PlaybackOrigin::Search { .. } => &icons::SEARCH,
        PlaybackOrigin::Recommendations { .. } => &icons::SEARCH,
    }
}

fn player_widget() -> impl Widget<Playback> {
    let buttons = Flex::row()
        .with_child(queue_behavior_widget())
        .with_spacer(theme::grid(1.5))
        .with_child(
            small_button_widget(&icons::SKIP_BACK).on_left_click(|ctx, _, _, _| {
                ctx.submit_command(cmd::PLAY_PREVIOUS);
            }),
        )
        .with_spacer(theme::grid(1.5))
        .with_child(player_play_pause_widget())
        .with_spacer(theme::grid(1.5))
        .with_child(
            small_button_widget(&icons::SKIP_FORWARD).on_left_click(|ctx, _, _, _| {
                ctx.submit_command(cmd::PLAY_NEXT);
            }),
        )
        .with_spacer(theme::grid(1.5))
        // Balances the queue behavior button so play/pause stays centered.
        .with_child(SizedBox::empty().width(theme::grid(4.5)));

    Flex::column()
        .with_child(buttons)
        .with_spacer(theme::grid(0.5))
        .with_child(Maybe::or_empty(progress_widget).lens(Playback::now_playing))
}

fn progress_widget() -> impl Widget<NowPlaying> {
    let elapsed = Label::dynamic(|np: &NowPlaying, _| utils::as_minutes_and_seconds(np.progress))
        .with_text_size(theme::TEXT_SIZE_SMALL)
        .with_text_color(theme::PLACEHOLDER_COLOR);
    let total = Label::dynamic(|np: &NowPlaying, _| {
        utils::as_minutes_and_seconds(np.item.duration())
    })
    .with_text_size(theme::TEXT_SIZE_SMALL)
    .with_text_color(theme::PLACEHOLDER_COLOR);

    Flex::row()
        .must_fill_main_axis(true)
        .with_child(elapsed.align_right().fix_width(theme::grid(5.0)))
        .with_spacer(theme::grid(1.0))
        .with_flex_child(SeekBar::new(), 1.0)
        .with_spacer(theme::grid(1.0))
        .with_child(total.align_left().fix_width(theme::grid(5.0)))
}

fn player_play_pause_widget() -> impl Widget<Playback> {
    ViewSwitcher::new(
        |playback: &Playback, _| playback.state,
        |state, _, _| match state {
            PlaybackState::Loading => Spinner::new()
                .with_color(theme::GREY_700)
                .fix_size(theme::grid(2.5), theme::grid(2.5))
                .padding(theme::grid(1.0))
                .background(theme::GREY_000)
                .rounded(f64::INFINITY)
                .on_left_click(|ctx, _, _, _| ctx.submit_command(cmd::PLAY_STOP))
                .boxed(),
            PlaybackState::Playing => icons::PAUSE
                .scale((theme::grid(2.5), theme::grid(2.5)))
                .with_color(theme::GREY_700)
                .padding(theme::grid(1.0))
                .background(theme::GREY_000)
                .rounded(f64::INFINITY)
                .on_left_click(|ctx, _, _, _| ctx.submit_command(cmd::PLAY_PAUSE))
                .boxed(),
            PlaybackState::Paused => icons::PLAY
                .scale((theme::grid(2.5), theme::grid(2.5)))
                .with_color(theme::GREY_700)
                .padding(theme::grid(1.0))
                .background(theme::GREY_000)
                .rounded(f64::INFINITY)
                .on_left_click(|ctx, _, _, _| ctx.submit_command(cmd::PLAY_RESUME))
                .boxed(),
            PlaybackState::Stopped => Empty.boxed(),
        },
    )
}

fn queue_behavior_widget() -> impl Widget<Playback> {
    ViewSwitcher::new(
        |playback: &Playback, _| playback.queue_behavior,
        |behavior, _, _| {
            faded_button_widget(queue_behavior_icon(behavior))
                .on_left_click(|ctx, _, playback: &mut Playback, _| {
                    ctx.submit_command(
                        cmd::PLAY_QUEUE_BEHAVIOR
                            .with(cycle_queue_behavior(&playback.queue_behavior)),
                    );
                })
                .boxed()
        },
    )
}

fn cycle_queue_behavior(qb: &QueueBehavior) -> QueueBehavior {
    match qb {
        QueueBehavior::Sequential => QueueBehavior::Random,
        QueueBehavior::Random => QueueBehavior::LoopTrack,
        QueueBehavior::LoopTrack => QueueBehavior::LoopAll,
        QueueBehavior::LoopAll => QueueBehavior::Sequential,
    }
}

fn queue_behavior_icon(qb: &QueueBehavior) -> &'static SvgIcon {
    match qb {
        QueueBehavior::Sequential => &icons::PLAY_SEQUENTIAL,
        QueueBehavior::Random => &icons::PLAY_SHUFFLE,
        QueueBehavior::LoopTrack => &icons::PLAY_LOOP_TRACK,
        QueueBehavior::LoopAll => &icons::PLAY_LOOP_ALL,
    }
}

fn small_button_widget<T: Data>(svg: &SvgIcon) -> impl Widget<T> {
    svg.scale((theme::grid(2.5), theme::grid(2.5)))
        .padding(theme::grid(1.0))
        .link()
        .rounded(theme::BUTTON_BORDER_RADIUS)
}

fn faded_button_widget<T: Data>(svg: &SvgIcon) -> impl Widget<T> {
    svg.scale((theme::grid(2.5), theme::grid(2.5)))
        .with_color(theme::PLACEHOLDER_COLOR)
        .padding(theme::grid(1.0))
        .link()
        .rounded(theme::BUTTON_BORDER_RADIUS)
}

struct SeekBar {
    loudness_path: BezPath,
}

impl SeekBar {
    fn new() -> Self {
        Self {
            loudness_path: BezPath::new(),
        }
    }
}

impl Widget<NowPlaying> for SeekBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, _data: &mut NowPlaying, _env: &Env) {
        match event {
            Event::MouseMove(_) => {
                ctx.set_cursor(&Cursor::Pointer);
            }
            Event::MouseDown(mouse) => {
                if mouse.button == MouseButton::Left {
                    ctx.set_active(true);
                }
            }
            Event::MouseUp(mouse)
                if ctx.is_active() && mouse.button == MouseButton::Left =>
            {
                if ctx.is_hot() {
                    let fraction = mouse.pos.x / ctx.size().width;
                    ctx.submit_command(cmd::PLAY_SEEK.with(fraction));
                }
                ctx.set_active(false);
            }
            _ => {}
        }
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        _data: &NowPlaying,
        _env: &Env,
    ) {
        match &event {
            LifeCycle::Size(_bounds) => {
                // self.loudness_path = compute_loudness_path(bounds, &data);
            }
            LifeCycle::HotChanged(_) => {
                ctx.request_paint();
            }
            _ => {}
        }
    }

    fn update(
        &mut self,
        ctx: &mut UpdateCtx,
        old_data: &NowPlaying,
        data: &NowPlaying,
        _env: &Env,
    ) {
        if !old_data.same(data) {
            ctx.request_paint();
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &NowPlaying,
        _env: &Env,
    ) -> Size {
        Size::new(bc.max().width, theme::grid(1.5))
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &NowPlaying, env: &Env) {
        if self.loudness_path.is_empty() {
            paint_progress_bar(ctx, data, env)
        } else {
            paint_audio_analysis(ctx, data, &self.loudness_path, env)
        }
    }
}

fn _compute_loudness_path_from_analysis(
    bounds: &Size,
    total_duration: &Duration,
    analysis: &AudioAnalysis,
) -> BezPath {
    let (loudness_min, loudness_max) = analysis
        .segments
        .iter()
        .map(|s| s.loudness_max)
        .minmax()
        .into_option()
        .unwrap_or((0.0, 0.0));
    let total_loudness = loudness_max - loudness_min;

    let mut path = BezPath::new();

    // We start in the middle of the vertical space and first draw the upper half of
    // the curve, then take what we have drawn, flip the y-axis and append it
    // underneath.
    let origin_y = bounds.height / 2.0;

    // Start at the origin.
    path.move_to((0.0, origin_y));

    // Because the size of the seekbar is quite small, but the number of the
    // segments can be large, we down-sample the loudness spectrum in a very
    // primitive way and only add a vertex after crossing `WIDTH_PRECISION` of
    // pixels horizontally.
    const WIDTH_PRECISION: f64 = 2.0;
    let mut last_width = 0.0;

    for seg in &analysis.segments {
        let time = seg.interval.start.as_secs_f64() + seg.loudness_max_time;
        let tfrac = time / total_duration.as_secs_f64();
        let width = bounds.width * tfrac;

        let loud = seg.loudness_max - loudness_min;
        let lfrac = loud / total_loudness;
        let height = bounds.height * lfrac;

        if width - last_width >= WIDTH_PRECISION {
            // Down-scale the height, because we will be drawing also the inverted half.
            path.line_to((width, origin_y - height / 2.0));

            // Save the X-coordinate of this vertex.
            last_width = width;
        }
    }

    // Land back at the vertical origin.
    path.line_to((bounds.width, origin_y));

    // Flip the y-axis, translate just under the origin, and append.
    let mut inverted_path = path.clone();
    let inversion_tx = Affine::FLIP_Y * Affine::translate((0.0, -bounds.height));
    inverted_path.apply_affine(inversion_tx);
    path.extend(inverted_path);

    path
}

fn paint_audio_analysis(ctx: &mut PaintCtx, data: &NowPlaying, path: &BezPath, env: &Env) {
    let bounds = ctx.size();

    let elapsed_time = data.progress.as_secs_f64();
    let total_time = data.item.duration().as_secs_f64();
    let elapsed_frac = elapsed_time / total_time;
    let elapsed_width = bounds.width * elapsed_frac;
    let elapsed = Size::new(elapsed_width, bounds.height).to_rect();

    let (elapsed_color, remaining_color) = if ctx.is_hot() {
        (env.get(theme::GREY_200), env.get(theme::GREY_500))
    } else {
        (env.get(theme::GREY_300), env.get(theme::GREY_600))
    };

    ctx.with_save(|ctx| {
        ctx.fill(path, &remaining_color);
        ctx.clip(elapsed);
        ctx.fill(path, &elapsed_color);
    });
}

fn paint_progress_bar(ctx: &mut PaintCtx, data: &NowPlaying, env: &Env) {
    const TRACK_HEIGHT: f64 = 4.0;
    const KNOB_RADIUS: f64 = 6.0;

    let elapsed_time = data.progress.as_secs_f64();
    let total_time = data.item.duration().as_secs_f64();
    let elapsed_frac = if total_time > 0.0 {
        (elapsed_time / total_time).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bounds = ctx.size();
    let is_hot = ctx.is_hot();
    let y = (bounds.height - TRACK_HEIGHT) / 2.0;
    let elapsed_width = bounds.width * elapsed_frac;

    // Like Spotify: a white bar that turns green while hovered, with a knob.
    let elapsed_color = if is_hot {
        env.get(theme::PRIMARY_DARK)
    } else {
        env.get(theme::GREY_100)
    };
    let track = Rect::new(0.0, y, bounds.width, y + TRACK_HEIGHT).to_rounded_rect(TRACK_HEIGHT);
    let elapsed =
        Rect::new(0.0, y, elapsed_width, y + TRACK_HEIGHT).to_rounded_rect(TRACK_HEIGHT);
    ctx.fill(track, &env.get(theme::GREY_500));
    ctx.fill(elapsed, &elapsed_color);

    if is_hot {
        let knob = druid::kurbo::Circle::new((elapsed_width, bounds.height / 2.0), KNOB_RADIUS);
        ctx.fill(knob, &env.get(theme::GREY_000));
    }
}
