"use strict";

import {
  request_episodes,
  request_shows,
  request_watch_status,
} from "./http.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a[1].name.toLowerCase() > b[1].name.toLowerCase());
}

function get_next_episode(episodes, today) {
  let next = null;
  let next_date = null;

  for (const i in episodes) {
    const this_date = Date.parse(episodes[i].airdate);

    if (today < this_date && (next_date === null || next_date > this_date)) {
      next = episodes[i];
      next_date = this_date;
    }
  }

  return next;
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

function remove_unaired_episodes(episodes, today) {
  return episodes.filter((episode) => {
    const show_date = Date.parse(episode.airdate);
    return show_date < today;
  });
}

async function populate_episodes() {
  let shows = await request_shows();
  let show_ids = [];
  let episode_promises = [];
  let watch_status_promises = [];
  for (let show_id in shows) {
    show_ids.push(show_id);
    episode_promises.push(request_episodes(show_id));
    watch_status_promises.push(request_watch_status(show_id));
  }

  let show_episodes = await Promise.all(episode_promises);
  let watched_episodes = await Promise.all(watch_status_promises);

  let next_up_shows = [];
  let finished_shows = [];
  let unstarted_shows = [];

  const today = Date.now();

  for (let i = 0; i < show_ids.length; i++) {
    const show_id = show_ids[i];
    const show = shows[show_id];
    const episodes = show_episodes[i];
    const watch_statuses = watched_episodes[i];

    const watch_status_keys = Object.keys(watch_statuses);
    if (watch_status_keys.length == 0) {
      unstarted_shows.push([show_id, show]);
      continue;
    }

    if (watch_status_keys.length == Object.keys(episodes).length) {
      finished_shows.push([show_id, show]);
      continue;
    }

    const unaired_episodes = remove_unaired_episodes(
      Object.values(episodes),
      today
    );
    if (watch_status_keys.length == unaired_episodes.length) {
      finished_shows.push([show_id, show]);
      continue;
    }

    next_up_shows.push([show_id, show]);
  }

  const finished_div = document.getElementById("finished");
  finished_div.innerHTML = render_shows(finished_shows);

  const unstarted_div = document.getElementById("unstarted");
  unstarted_div.innerHTML = render_shows(unstarted_shows);

  const next_up_div = document.getElementById("next-up");
  next_up_div.innerHTML = render_shows(next_up_shows);
}

async function init() {
  populate_episodes();
}

init();
