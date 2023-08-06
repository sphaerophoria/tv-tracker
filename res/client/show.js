"use strict";

import { request_episodes, request_shows } from "./http.js";

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
    this_season_episodes.push(episode);
  }

  return season_episodes;
}

function render_show(show, episodes) {
  let ret = "<h1>" + show.name + "</h1>";

  let season_episodes = group_episodes_by_seasons(episodes);

  let today = Date.now();

  for (const [season, episodes] of season_episodes) {
    ret += "<h2>Season " + season + "</h2>";
    for (const episode of episodes) {
      let aired_class = "unaired";
      if (Date.parse(episode.airdate) < today) {
        aired_class = "aired";
      }

      ret += '<div class="' + aired_class + '">';
      ret += " " + episode.airdate;
      ret += " Episode " + episode.episode;
      ret += ": ";

      ret += episode.name;

      ret += "</div>";
    }
  }

  return ret;
}

async function init() {
  const show_id = page_show_id();

  const shows_promise = request_shows();
  const episodes_promise = request_episodes(show_id);

  let shows_json, episodes_json;
  [shows_json, episodes_json] = await Promise.all([
    shows_promise,
    episodes_promise,
  ]);

  const show_div = document.getElementById("show");
  show_div.innerHTML = render_show(shows_json[show_id], episodes_json);
}

init();
