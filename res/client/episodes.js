"use strict";

import {
  request_shows,
  request_shows_by_watch_status,
  request_paused_shows,
} from "./http.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a[1].name.toLowerCase() > b[1].name.toLowerCase());
}

function render_shows(shows) {
  sort_shows_by_name(shows);

  let ret = "";
  for (const item of shows) {
    let show_id, show;
    [show_id, show] = item;
    ret += "<a href=/show.html?show_id=" + show_id + ">";
    ret += show.name;
    ret += "</a>";
    ret += "<br>";
  }
  return ret;
}

async function populate_episodes() {
  let shows_promise = request_shows();
  let show_statuses_promise = request_shows_by_watch_status();
  let paused_shows_promise = request_paused_shows();

  let shows, show_statuses, paused_show_ids;
  [shows, show_statuses, paused_show_ids] = await Promise.all([
    shows_promise,
    show_statuses_promise,
    paused_shows_promise,
  ]);

  let next_up_shows = [];
  let finished_shows = [];
  let unstarted_shows = [];
  let paused_shows = [];

  for (const show_id in shows) {
    const show = shows[show_id];
    const show_status = show_statuses[show_id];

    if (paused_show_ids.includes(parseInt(show_id))) {
      paused_shows.push([show_id, show]);
    } else if (show_status == "finished") {
      finished_shows.push([show_id, show]);
    } else if (show_status == "in_progress") {
      next_up_shows.push([show_id, show]);
    } else if (show_status == "unstarted") {
      unstarted_shows.push([show_id, show]);
    } else {
      throw new Error("Invalid show status " + show_status);
    }
  }

  const finished_div = document.getElementById("finished");
  finished_div.innerHTML = render_shows(finished_shows);

  const unstarted_div = document.getElementById("unstarted");
  unstarted_div.innerHTML = render_shows(unstarted_shows);

  const next_up_div = document.getElementById("next-up");
  next_up_div.innerHTML = render_shows(next_up_shows);

  const paused_div = document.getElementById("paused");
  paused_div.innerHTML = render_shows(paused_shows);
}

async function init() {
  populate_episodes();
}

init();
