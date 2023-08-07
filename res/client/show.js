"use strict";

import {
  request_episodes,
  request_shows,
  request_watch_status,
  request_set_watch_status,
} from "./http.js";

function page_show_id() {
  const params = new URLSearchParams(document.location.search);
  return params.get("show_id");
}

function group_episodes_by_seasons(episodes) {
  let season_episodes = new Map();

  for (const i in episodes) {
    const episode = episodes[i];
    if (!season_episodes.has(episode.season)) {
      season_episodes.set(episode.season, []);
    }

    let this_season_episodes = season_episodes.get(episode.season);
    this_season_episodes.push([i, episode]);
  }

  return season_episodes;
}

async function set_show_watch_status(episode_id, watched) {
  if (watched) {
    request_set_watch_status(episode_id, null);
  } else {
    const now = new Date(Date.now());
    let date_string = now.toISOString().substring(0, 10);

    request_set_watch_status(episode_id, date_string);
  }

  init();
}

function render_show(show, episodes, watch_status) {
  let ret = "<h1>" + show.name + "</h1>";

  let season_episodes = group_episodes_by_seasons(episodes);

  let today = Date.now();

  for (const [season, episodes] of season_episodes) {
    ret += "<h2>Season " + season + "</h2>";
    for (let [episode_id, episode] of episodes) {
      let aired_class = "unaired";
      if (Date.parse(episode.airdate) < today) {
        aired_class = "aired";
      }

      let watched_class = "unwatched";
      const episode_watched = episode_id in watch_status;
      if (episode_watched) {
        watched_class = "watched";
      }

      console.log(episode_id);
      ret += "<a href=#/ ";
      ret += 'class="' + aired_class + " " + watched_class + '"';
      ret +=
        ' onclick="set_show_watch_status(' +
        episode_id +
        "," +
        episode_watched +
        ')" ';
      ret += ">";

      ret += " " + episode.airdate;
      ret += " Episode " + episode.episode;
      ret += ": ";

      ret += episode.name;

      ret += "</a>";
      ret += "<br>";
    }
  }

  return ret;
}

async function init() {
  window.set_show_watch_status = set_show_watch_status;

  const show_id = page_show_id();

  const shows_promise = request_shows();
  const episodes_promise = request_episodes(show_id);
  const watch_status_promise = request_watch_status(show_id);

  let shows_json, episodes_json, watch_status_json;
  [shows_json, episodes_json, watch_status_json] = await Promise.all([
    shows_promise,
    episodes_promise,
    watch_status_promise,
  ]);
  console.log(watch_status_json);

  const show_div = document.getElementById("show");
  show_div.innerHTML = render_show(
    shows_json[show_id],
    episodes_json,
    watch_status_json
  );
}

init();
