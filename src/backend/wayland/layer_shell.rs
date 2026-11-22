use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat},
};

use crate::core::bar::{Bar, render_bar};
use crate::core::event::{ClickEvent, MouseButton};
use crate::core::layout::BarLayout;
use crate::core::module::ModuleId;
use crate::core::popup::{POPUP_GAP, PopupLayout, PopupMenu, draw_popup, layout_popup};
use crate::renderer::cairo_renderer::CairoRenderer;
use crate::renderer::primitives::Renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerFocus {
    None,
    Bar,
    Popup,
    Catcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RevealZoneSide {
    Left,
    Right,
}

impl RevealZoneSide {
    fn from_config(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "left" => Self::Left,
            "right" => Self::Right,
            other => {
                log::warn!("Unknown autohide_zone_side '{other}'; using 'right'");
                Self::Right
            }
        }
    }
}

struct PopupSurfaceState {
    layer_surface: LayerSurface,
    pool: SlotPool,
    renderer: CairoRenderer,
    owner: ModuleId,
    menu: PopupMenu,
    layout: PopupLayout,
    configured: bool,
    hovered_item: Option<usize>,
}

struct PopupCatcherState {
    layer_surface: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
}

struct WaylandState {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    queue_handle: QueueHandle<WaylandState>,
    shm: Shm,
    pool: SlotPool,
    layer_surface: LayerSurface,
    popup: Option<PopupSurfaceState>,
    popup_catcher: Option<PopupCatcherState>,
    bar: Bar,
    renderer: CairoRenderer,
    configured: bool,
    width: u32,
    height: u32,
    exit: bool,
    needs_redraw: bool,
    pointer: Option<wl_pointer::WlPointer>,
    pointer_pos: (f64, f64),
    pointer_focus: PointerFocus,
    autohide: bool,
    autohide_delay: Duration,
    autohide_show_delay: Duration,
    autohide_reveal: u32,
    reveal_zone: u32,
    reveal_zone_side: RevealZoneSide,
    bottom: bool,
    hidden: bool,
    compact: bool,
    hide_after: Option<Instant>,
    show_after: Option<Instant>,
}

pub fn run_layer_shell(bar: &mut Bar) {
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, mut event_queue) = registry_queue_init(&conn).expect("Failed to init registry");
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("No wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("No wlr_layer_shell");
    let shm = Shm::bind(&globals, &qh).expect("No wl_shm");

    let surface = compositor.create_surface(&qh);

    let height = bar.height;
    let width = bar.width;
    let bottom = bar.bottom;
    let autohide = bar.autohide;
    let autohide_delay = bar.autohide_delay;
    let autohide_show_delay = bar.autohide_show_delay;
    let autohide_reveal = bar.autohide_reveal;
    let reveal_zone = if bar.autohide_zone_enabled {
        bar.autohide_reveal_zone
    } else {
        0
    };
    let reveal_zone_side = RevealZoneSide::from_config(&bar.autohide_zone_side);
    let refresh_ceiling = bar.refresh_interval;

    let layer_surface =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("raidzbar"), None);
    if bottom {
        layer_surface.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    } else {
        layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    }
    layer_surface.set_size(0, height);
    layer_surface.set_exclusive_zone(height as i32);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.commit();

    let pool =
        SlotPool::new((3840 * height * 4) as usize, &shm).expect("Failed to create slot pool");

    let _seat: Option<wl_seat::WlSeat> = globals
        .bind::<wl_seat::WlSeat, _, _>(&qh, 1..=8, ())
        .map_err(|e| log::warn!("[wayland] no wl_seat: {e}"))
        .ok();

    let bar_owned = std::mem::replace(bar, Bar::new(0, 0));

    let wayland_fd = {
        use std::os::unix::io::AsFd;
        event_queue.as_fd().as_raw_fd()
    };

    conn.flush()
        .expect("Failed to flush initial Wayland requests");

    let mut state = WaylandState {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        queue_handle: qh.clone(),
        shm,
        pool,
        layer_surface,
        popup: None,
        popup_catcher: None,
        bar: bar_owned,
        renderer: CairoRenderer::new(width, height),
        configured: false,
        width,
        height,
        exit: false,
        needs_redraw: false,
        pointer: None,
        pointer_pos: (0.0, 0.0),
        pointer_focus: PointerFocus::None,
        autohide,
        autohide_delay,
        autohide_show_delay,
        autohide_reveal,
        reveal_zone,
        reveal_zone_side,
        bottom,
        hidden: false,
        compact: false,
        hide_after: None,
        show_after: None,
    };

    event_queue
        .roundtrip(&mut state)
        .expect("initial roundtrip failed");

    let mut next_tick = std::time::Instant::now();

    'main: loop {
        if let Err(e) = conn.flush() {
            log::error!("Wayland connection lost (flush): {e}");
            break;
        }

        let now = Instant::now();
        let mut deadline = next_tick;
        if let Some(hide_after) = state.hide_after
            && state.pointer_focus == PointerFocus::None
            && state.popup.is_none()
        {
            deadline = deadline.min(hide_after);
        }
        if let Some(show_after) = state.show_after {
            deadline = deadline.min(show_after);
        }
        let timeout_ms: i32 = if now >= deadline {
            0
        } else {
            (deadline - now).as_millis().min(i32::MAX as u128) as i32
        };

        let guard = event_queue.prepare_read();

        let mut pollfd = libc::pollfd {
            fd: wayland_fd,
            events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
            revents: 0,
        };
        let poll_ret = unsafe { libc::poll(&mut pollfd as *mut libc::pollfd, 1, timeout_ms) };

        if poll_ret > 0
            && pollfd.revents & (libc::POLLHUP | libc::POLLERR) != 0
            && pollfd.revents & libc::POLLIN == 0
        {
            log::error!("Wayland compositor disconnected (poll HUP/ERR)");
            drop(guard);
            break;
        }

        if poll_ret > 0 && pollfd.revents & libc::POLLIN != 0 {
            if let Some(g) = guard {
                match g.read() {
                    Ok(_) => {}
                    Err(wayland_client::backend::WaylandError::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        log::error!("Wayland read failed: {e}");
                        break 'main;
                    }
                }
            }
        } else {
            drop(guard);
        }

        if let Err(e) = event_queue.dispatch_pending(&mut state) {
            log::error!("Wayland dispatch failed: {e}");
            break;
        }

        let now = Instant::now();
        autohide_tick(&mut state, now);
        let timer_fired = state.configured && now >= next_tick;

        let mut should_redraw = state.needs_redraw;
        if timer_fired {
            let tick = state.bar.tick();
            let ceiling = now + refresh_ceiling;
            next_tick = match tick.next_due {
                Some(due) => due.min(ceiling),
                None => ceiling,
            };
            should_redraw = true;
        }

        if should_redraw {
            state.needs_redraw = false;
            sync_popup_surface(&mut state);
            draw_frame(&mut state);
            if let Err(e) = conn.flush() {
                log::error!("Wayland connection lost (flush after draw): {e}");
                break;
            }
        }

        if state.exit {
            break;
        }
    }
}

fn reveal_bar(state: &mut WaylandState) {
    if !state.hidden && state.hide_after.is_none() && state.show_after.is_none() {
        return;
    }
    state.hidden = false;
    state.compact = false;
    state.hide_after = None;
    state.show_after = None;
    log::debug!("[autohide] revealing bar");
    let edge = if state.bottom {
        Anchor::BOTTOM
    } else {
        Anchor::TOP
    };
    state
        .layer_surface
        .set_anchor(edge | Anchor::LEFT | Anchor::RIGHT);
    state.layer_surface.set_size(0, state.height);
    state.layer_surface.set_margin(0, 0, 0, 0);
    state.layer_surface.set_exclusive_zone(state.height as i32);
    state.layer_surface.wl_surface().commit();
}

fn hide_bar(state: &mut WaylandState) {
    if state.hidden {
        return;
    }
    state.hidden = true;
    state.compact = true;
    state.show_after = None;
    log::debug!("[autohide] hiding bar");

    let full_width = state.reveal_zone == 0;
    let zone = if full_width {
        state.width
    } else {
        state.reveal_zone.min(state.width)
    };
    let reveal = state.autohide_reveal.max(1).min(state.height);

    let edge = if state.bottom {
        Anchor::BOTTOM
    } else {
        Anchor::TOP
    };
    let anchor = if full_width {
        edge | Anchor::LEFT | Anchor::RIGHT
    } else if state.reveal_zone_side == RevealZoneSide::Left {
        edge | Anchor::LEFT
    } else {
        edge | Anchor::RIGHT
    };
    state.layer_surface.set_anchor(anchor);
    if full_width {
        state.layer_surface.set_size(0, reveal);
    } else {
        state.layer_surface.set_size(zone, reveal);
    }
    state.layer_surface.set_margin(0, 0, 0, 0);
    state.layer_surface.set_exclusive_zone(0);
    state.layer_surface.wl_surface().commit();
}

fn request_show(state: &mut WaylandState) {
    state.hide_after = None;
    if !state.autohide || !state.hidden {
        state.show_after = None;
        return;
    }
    if state.autohide_show_delay.is_zero() {
        state.show_after = None;
        reveal_bar(state);
    } else {
        state.show_after = Some(Instant::now() + state.autohide_show_delay);
    }
}

fn in_reveal_zone(state: &WaylandState, x: f64) -> bool {
    if state.compact || state.reveal_zone == 0 {
        return true;
    }
    let zone = state.reveal_zone as f64;
    let width = state.width as f64;
    match state.reveal_zone_side {
        RevealZoneSide::Left => x <= zone,
        RevealZoneSide::Right => x >= width - zone,
    }
}

fn schedule_hide(state: &mut WaylandState) {
    if state.autohide && !state.hidden {
        state.hide_after = Some(Instant::now() + state.autohide_delay);
    }
}

fn autohide_tick(state: &mut WaylandState, now: Instant) {
    if !state.autohide {
        return;
    }

    if let Some(show_at) = state.show_after
        && now >= show_at
    {
        state.show_after = None;
        if state.pointer_focus != PointerFocus::None {
            reveal_bar(state);
        }
    }

    if let Some(deadline) = state.hide_after
        && now >= deadline
    {
        if state.pointer_focus != PointerFocus::None || state.popup.is_some() {
            state.hide_after = None;
        } else {
            state.hide_after = None;
            hide_bar(state);
        }
    }

    if !state.hidden
        && state.hide_after.is_none()
        && state.pointer_focus == PointerFocus::None
        && state.popup.is_none()
    {
        state.hide_after = Some(now + state.autohide_delay);
    }
}

fn draw_frame(state: &mut WaylandState) {
    let bar_width = state.width;
    let bar_height = state.height;

    let (surface_width, surface_height, crop_x, crop_y) = if state.compact {
        let full_width = state.reveal_zone == 0;
        let zone = if full_width {
            bar_width
        } else {
            state.reveal_zone.min(bar_width)
        };
        let reveal = state.autohide_reveal.max(1).min(bar_height);
        let crop_x = if full_width || state.reveal_zone_side == RevealZoneSide::Right {
            bar_width - zone
        } else {
            0
        };
        let crop_y = if state.bottom { 0 } else { bar_height - reveal };
        (zone, reveal, crop_x, crop_y)
    } else {
        (bar_width, bar_height, 0, 0)
    };

    let stride = surface_width * 4;
    let buf_size = (stride * surface_height) as usize;
    state.pool.resize(buf_size).expect("Failed to resize pool");

    let (buffer, canvas) = state
        .pool
        .create_buffer(
            surface_width as i32,
            surface_height as i32,
            stride as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        )
        .expect("Failed to create buffer");

    render_bar(&state.bar, &mut state.renderer, bar_width, bar_height);

    let data = state.renderer.data();
    for row in 0..surface_height {
        let src = ((crop_y + row) * bar_width + crop_x) as usize * 4;
        let dst = (row * surface_width) as usize * 4;
        let len = (surface_width * 4) as usize;
        if src + len <= data.len() && dst + len <= canvas.len() {
            canvas[dst..dst + len].copy_from_slice(&data[src..src + len]);
        }
    }

    state
        .layer_surface
        .wl_surface()
        .attach(Some(buffer.wl_buffer()), 0, 0);
    state.layer_surface.wl_surface().damage_buffer(
        0,
        0,
        surface_width as i32,
        surface_height as i32,
    );
    state.layer_surface.wl_surface().commit();
}

fn sync_popup_surface(state: &mut WaylandState) {
    let had_popup = state.popup.is_some();
    let active_popup = state.bar.active_popup().and_then(|(owner, menu)| {
        layout_popup(
            &menu,
            state.width as f64,
            &state.bar.text_style,
            &state.renderer,
        )
        .map(|layout| (owner, menu, layout))
    });

    match active_popup {
        Some((owner, menu, layout)) => {
            reveal_bar(state);
            let popup_width = layout.pixel_width();
            let popup_height = layout.pixel_height();
            let left_margin = layout.x.round() as i32;
            let edge_margin = POPUP_GAP.round() as i32;

            if state.popup_catcher.is_none() {
                create_popup_catcher(state);
            }

            if state.popup.is_none() {
                let surface = state.compositor.create_surface(&state.queue_handle);
                let layer_surface = state.layer_shell.create_layer_surface(
                    &state.queue_handle,
                    surface,
                    Layer::Overlay,
                    Some("raidzbar-popup"),
                    None,
                );
                if state.bar.bottom {
                    layer_surface.set_anchor(Anchor::BOTTOM | Anchor::LEFT);
                } else {
                    layer_surface.set_anchor(Anchor::TOP | Anchor::LEFT);
                }
                layer_surface.set_exclusive_zone(0);
                layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

                let pool = SlotPool::new(
                    (popup_width.max(1) * popup_height.max(1) * 4) as usize,
                    &state.shm,
                )
                .expect("Failed to create Wayland popup slot pool");

                state.popup = Some(PopupSurfaceState {
                    layer_surface,
                    pool,
                    renderer: CairoRenderer::new(popup_width, popup_height),
                    owner: owner.clone(),
                    menu: menu.clone(),
                    layout: layout.clone(),
                    configured: false,
                    hovered_item: None,
                });
            }

            let popup = state.popup.as_mut().unwrap();
            popup.owner = owner;
            popup.menu = menu;
            popup.layout = layout;
            popup.layer_surface.set_size(popup_width, popup_height);
            if state.bar.bottom {
                popup
                    .layer_surface
                    .set_margin(0, 0, edge_margin, left_margin);
            } else {
                popup
                    .layer_surface
                    .set_margin(edge_margin, 0, 0, left_margin);
            }

            if popup.configured {
                draw_popup_frame(popup, &state.bar.text_style);
            } else {
                popup.layer_surface.wl_surface().commit();
            }
        }
        None => {
            state.popup = None;
            state.popup_catcher = None;
            if matches!(
                state.pointer_focus,
                PointerFocus::Popup | PointerFocus::Catcher
            ) {
                state.pointer_focus = PointerFocus::None;
            }
            if had_popup {
                schedule_hide(state);
            }
        }
    }
}

fn create_popup_catcher(state: &mut WaylandState) {
    let surface = state.compositor.create_surface(&state.queue_handle);
    let layer_surface = state.layer_shell.create_layer_surface(
        &state.queue_handle,
        surface,
        Layer::Top,
        Some("raidzbar-popup-catcher"),
        None,
    );
    layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer_surface.set_size(0, 0);
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.commit();

    let pool = SlotPool::new(64 * 64 * 4, &state.shm)
        .expect("Failed to create Wayland popup catcher slot pool");

    state.popup_catcher = Some(PopupCatcherState {
        layer_surface,
        pool,
        width: 0,
        height: 0,
        configured: false,
    });
}

fn draw_popup_catcher_frame(catcher: &mut PopupCatcherState) {
    let width = catcher.width.max(1);
    let height = catcher.height.max(1);
    let stride = width * 4;
    let buf_size = (stride * height) as usize;

    catcher
        .pool
        .resize(buf_size)
        .expect("Failed to resize Wayland popup catcher slot pool");

    let (buffer, canvas) = catcher
        .pool
        .create_buffer(
            width as i32,
            height as i32,
            stride as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        )
        .expect("Failed to create Wayland popup catcher buffer");

    let copy_len = buf_size.min(canvas.len());
    for byte in &mut canvas[..copy_len] {
        *byte = 0;
    }

    catcher
        .layer_surface
        .wl_surface()
        .attach(Some(buffer.wl_buffer()), 0, 0);
    catcher
        .layer_surface
        .wl_surface()
        .damage_buffer(0, 0, width as i32, height as i32);
    catcher.layer_surface.wl_surface().commit();
}

fn draw_popup_frame(
    popup: &mut PopupSurfaceState,
    text_style: &crate::renderer::primitives::TextStyle,
) {
    let width = popup.layout.pixel_width();
    let height = popup.layout.pixel_height();
    let stride = width * 4;
    let buf_size = (stride * height) as usize;

    popup
        .pool
        .resize(buf_size)
        .expect("Failed to resize Wayland popup slot pool");

    let (buffer, canvas) = popup
        .pool
        .create_buffer(
            width as i32,
            height as i32,
            stride as i32,
            wayland_client::protocol::wl_shm::Format::Argb8888,
        )
        .expect("Failed to create Wayland popup buffer");

    popup.renderer.begin(width, height);
    draw_popup(
        &mut popup.renderer,
        &popup.menu,
        &popup.layout,
        text_style,
        popup.hovered_item,
    );
    popup.renderer.end();

    let data = popup.renderer.data();
    let copy_len = buf_size.min(data.len()).min(canvas.len());
    canvas[..copy_len].copy_from_slice(&data[..copy_len]);

    popup
        .layer_surface
        .wl_surface()
        .attach(Some(buffer.wl_buffer()), 0, 0);
    popup
        .layer_surface
        .wl_surface()
        .damage_buffer(0, 0, width as i32, height as i32);
    popup.layer_surface.wl_surface().commit();
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wayland_client::protocol::wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        if *_layer == self.layer_surface {
            self.exit = true;
        } else if self
            .popup
            .as_ref()
            .is_some_and(|popup| popup.layer_surface == *_layer)
            || self
                .popup_catcher
                .as_ref()
                .is_some_and(|catcher| catcher.layer_surface == *_layer)
        {
            self.bar.dismiss_all_popups();
            self.popup = None;
            self.popup_catcher = None;
            self.pointer_focus = PointerFocus::None;
        }
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if *_layer == self.layer_surface {
            log::debug!("LayerSurface configure: new_size={:?}", configure.new_size);
            // While compact, configure describes the tiny stub, not the bar.
            // Keep the bar's logical geometry so layout/popups stay correct.
            if !self.compact {
                if configure.new_size.0 > 0 {
                    self.width = configure.new_size.0;
                }
                if configure.new_size.1 > 0 {
                    self.height = configure.new_size.1;
                }
                self.bar.width = self.width;
            }
            self.configured = true;
            self.bar.update_all();
            sync_popup_surface(self);
            draw_frame(self);
        } else if let Some(popup) = self
            .popup
            .as_mut()
            .filter(|popup| popup.layer_surface == *_layer)
        {
            popup.configured = true;
            draw_popup_frame(popup, &self.bar.text_style);
        } else if let Some(catcher) = self
            .popup_catcher
            .as_mut()
            .filter(|catcher| catcher.layer_surface == *_layer)
        {
            if configure.new_size.0 > 0 {
                catcher.width = configure.new_size.0;
            }
            if configure.new_size.1 > 0 {
                catcher.height = configure.new_size.1;
            }
            catcher.configured = true;
            draw_popup_catcher_frame(catcher);
        }
    }
}

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            // Pointer capability bit = 1 (per Wayland protocol spec).
            let has_pointer = match capabilities {
                WEnum::Value(c) => (u32::from(c) & 1) != 0,
                WEnum::Unknown(n) => (n & 1) != 0,
            };
            if has_pointer && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
                log::debug!("[wayland] pointer acquired");
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_pos = (surface_x, surface_y);
                state.pointer_focus = if surface == *state.layer_surface.wl_surface() {
                    PointerFocus::Bar
                } else if state
                    .popup
                    .as_ref()
                    .is_some_and(|popup| surface == *popup.layer_surface.wl_surface())
                {
                    PointerFocus::Popup
                } else if state
                    .popup_catcher
                    .as_ref()
                    .is_some_and(|catcher| surface == *catcher.layer_surface.wl_surface())
                {
                    PointerFocus::Catcher
                } else {
                    PointerFocus::None
                };
                if state.pointer_focus != PointerFocus::None && in_reveal_zone(state, surface_x) {
                    request_show(state);
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_pos = (surface_x, surface_y);
                if state.hidden
                    && state.pointer_focus == PointerFocus::Bar
                    && in_reveal_zone(state, surface_x)
                {
                    request_show(state);
                }
                if state.pointer_focus == PointerFocus::Popup {
                    let hover_changed = if let Some(popup) = state.popup.as_mut() {
                        let new_hover =
                            popup
                                .layout
                                .hit_test(&popup.menu.items, surface_x, surface_y);
                        let changed = new_hover != popup.hovered_item;
                        popup.hovered_item = new_hover;
                        changed
                    } else {
                        false
                    };
                    if hover_changed {
                        let style = state.bar.text_style.clone();
                        if let Some(popup) = state.popup.as_mut().filter(|p| p.configured) {
                            draw_popup_frame(popup, &style);
                            conn.flush().ok();
                        }
                    }
                }
            }
            wl_pointer::Event::Leave { .. } => {
                if state.pointer_focus == PointerFocus::Popup {
                    let style = state.bar.text_style.clone();
                    if let Some(popup) = state.popup.as_mut().filter(|p| p.configured) {
                        popup.hovered_item = None;
                        draw_popup_frame(popup, &style);
                        conn.flush().ok();
                    }
                }
                state.pointer_focus = PointerFocus::None;
                state.show_after = None;
                schedule_hide(state);
            }
            wl_pointer::Event::Button {
                button,
                state: btn_state,
                ..
            } => {
                if !matches!(btn_state, WEnum::Value(wl_pointer::ButtonState::Pressed)) {
                    return;
                }
                if !state.configured {
                    return;
                }

                let mb = match button {
                    0x110 => MouseButton::Left,   // BTN_LEFT
                    0x111 => MouseButton::Right,  // BTN_RIGHT
                    0x112 => MouseButton::Middle, // BTN_MIDDLE
                    n => MouseButton::Other(n),
                };

                if state.pointer_focus == PointerFocus::Bar {
                    request_show(state);
                }
                dispatch_pointer_event(state, mb);
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                if !state.configured {
                    return;
                }
                if !matches!(axis, WEnum::Value(wl_pointer::Axis::VerticalScroll)) {
                    return;
                }
                if value == 0.0 {
                    return;
                }
                let mb = if value > 0.0 {
                    MouseButton::ScrollDown
                } else {
                    MouseButton::ScrollUp
                };
                if state.pointer_focus != PointerFocus::Bar {
                    return;
                }
                dispatch_pointer_event(state, mb);
            }
            _ => {}
        }
    }
}

fn dispatch_pointer_event(state: &mut WaylandState, mb: MouseButton) {
    let (px, py) = state.pointer_pos;

    if state.pointer_focus == PointerFocus::Popup {
        if let Some((owner, item_idx)) = state.popup.as_ref().and_then(|popup| {
            popup
                .layout
                .hit_test(&popup.menu.items, px, py)
                .map(|item_idx| (popup.owner.clone(), item_idx))
        }) {
            state.bar.handle_popup_click(&owner, item_idx, mb);
        } else {
            state.bar.dismiss_all_popups();
        }
        state.bar.update_all();
        state.needs_redraw = true;
        return;
    }

    if state.pointer_focus == PointerFocus::Catcher {
        state.bar.dismiss_all_popups();
        state.bar.update_all();
        state.needs_redraw = true;
        return;
    }

    if state.popup.is_some() {
        state.bar.dismiss_all_popups();
    }

    let click = ClickEvent {
        x: px,
        bar_x: px,
        screen_x: px,
        module_width: 0.0,
        y: py,
        bar_y: py,
        screen_y: py,
        button: mb,
    };

    let width = state.width;
    let groups = state.bar.compute_groups(width as f64, &state.renderer);
    let regions = BarLayout::flatten_modules(&groups);
    state.bar.handle_click(&regions, &click);
    state.needs_redraw = true;
}

delegate_compositor!(WaylandState);
delegate_output!(WaylandState);
delegate_layer!(WaylandState);
delegate_shm!(WaylandState);
delegate_registry!(WaylandState);
