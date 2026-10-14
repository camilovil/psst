use druid::{
    widget::{CrossAxisAlignment, Flex, Label, LineBreaking, Scroll},
    LensExt, Size, Widget, WidgetExt,
};

use crate::{
    cmd::{self, SHOW_ARTWORK, TOGGLE_LYRICS},
    data::{AppState, NowPlaying, Playable, Playback},
    widget::{Maybe, MyWidgetExt, RemoteImage},
};

use super::{playback, theme, utils};

/// Width of the "Now Playing" panel docked to the right of the window.
pub const PANEL_WIDTH: f64 = 288.0;
const PADDING: f64 = theme::GRID * 2.0;
const COVER_SIZE: f64 = PANEL_WIDTH - 2.0 * PADDING;

pub fn panel_widget() -> impl Widget<AppState> {
    Scroll::new(
        Maybe::or_empty(now_playing_widget).lens(AppState::playback.then(Playback::now_playing)),
    )
    .vertical()
    .expand_height()
    .background(theme::BACKGROUND_LIGHT)
    .rounded(theme::PANEL_RADIUS)
}

fn now_playing_widget() -> impl Widget<NowPlaying> {
    let origin = Label::dynamic(|np: &NowPlaying, _| np.origin.to_string())
        .with_font(theme::UI_FONT_MEDIUM)
        .with_text_size(theme::TEXT_SIZE_LARGE)
        .with_line_break_mode(LineBreaking::Clip)
        .on_click(|ctx, np: &mut NowPlaying, _| {
            ctx.submit_command(cmd::NAVIGATE.with(np.origin.to_nav()));
        });

    let cover = RemoteImage::new(utils::placeholder_widget(), |np: &NowPlaying, _| {
        np.cover_image_url(COVER_SIZE, COVER_SIZE).map(Into::into)
    })
    .fix_size(COVER_SIZE, COVER_SIZE)
    .clip(Size::new(COVER_SIZE, COVER_SIZE).to_rounded_rect(theme::cover_radius(COVER_SIZE)))
    .on_left_click(|ctx, _, _, _| ctx.submit_command(SHOW_ARTWORK));

    let title = Label::dynamic(|np: &NowPlaying, _| np.item.name().to_string())
        .with_font(theme::UI_FONT_MEDIUM)
        .with_text_size(22.0)
        .with_line_break_mode(LineBreaking::WordWrap);

    let subtitle = Label::dynamic(|np: &NowPlaying, _| match &np.item {
        Playable::Track(track) => track.artist_names(),
        Playable::Episode(episode) => episode.show.name.to_string(),
    })
    .with_text_color(theme::PLACEHOLDER_COLOR)
    .with_line_break_mode(LineBreaking::WordWrap);

    let details = Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(
            Flex::column()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(title)
                .with_spacer(theme::grid(0.5))
                .with_child(subtitle),
            1.0,
        )
        .with_child(playback::save_button_widget(theme::ICON_SIZE_LARGE));

    let album = Label::dynamic(|np: &NowPlaying, _| match &np.item {
        Playable::Track(track) => format!("From the album {}", track.album_name()),
        Playable::Episode(_) => String::new(),
    })
    .with_text_size(theme::TEXT_SIZE_SMALL)
    .with_text_color(theme::PLACEHOLDER_COLOR)
    .with_line_break_mode(LineBreaking::WordWrap);

    let actions = Flex::row()
        .with_child(pill_button("Lyrics").on_left_click(|ctx, _, _, _| {
            ctx.submit_command(TOGGLE_LYRICS);
        }))
        .with_spacer(theme::GRID)
        .with_child(
            pill_button("Credits").on_left_click(|ctx, _, np: &mut NowPlaying, _| {
                if let Some(track) = np.item.track() {
                    ctx.submit_command(cmd::SHOW_CREDITS_WINDOW.with(track.clone()));
                }
            }),
        );

    Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(origin)
        .with_spacer(theme::grid(2.0))
        .with_child(cover)
        .with_spacer(theme::grid(2.0))
        .with_child(details)
        .with_spacer(theme::grid(1.0))
        .with_child(album)
        .with_spacer(theme::grid(2.0))
        .with_child(actions)
        .padding(PADDING)
}

fn pill_button(text: &str) -> impl Widget<NowPlaying> {
    Label::new(text)
        .with_font(theme::UI_FONT_MEDIUM)
        .padding((theme::grid(2.0), theme::grid(1.0)))
        .link()
        .rounded(f64::INFINITY)
        .border(theme::GREY_400, 1.0)
        .rounded(f64::INFINITY)
}
