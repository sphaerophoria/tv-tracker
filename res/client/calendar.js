"use strict";

import { get_shows, get_episodes } from "./http.js";
import { render_card_element } from "./show_card.js";

function date_to_string(date) {
  return date.toISOString().substring(0, 10);
}

function sort_by_date(episodes) {
  episodes.sort((a, b) => {
    return new Date(a.airdate) > new Date(b.airdate);
  });
}

function render_date_shows(date_shows, date_it, parent) {
  let header = document.createElement("h1");
  header.innerText = date_it;
  parent.appendChild(header);

  let div = document.createElement("div");
  div.classList.add("show-list");
  parent.appendChild(div);

  for (const show of date_shows) {
    const href = "/show.html?show_id=" + show.id;
    let elem = render_card_element(show, href, []);
    div.appendChild(elem);
  }
}

async function init() {
  const today = new Date(Date.now());
  const month_start = new Date(today.getUTCFullYear(), today.getMonth(), 1);
  const month_end = new Date(today.getUTCFullYear(), today.getMonth() + 1, 0);

  let shows_promise = get_shows();
  let episodes_promise = get_episodes(
    date_to_string(month_start),
    date_to_string(month_end)
  );

  let shows, episodes;
  [shows, episodes] = await Promise.all([shows_promise, episodes_promise]);
  episodes = Object.values(episodes);
  sort_by_date(episodes);

  let date_it = null;
  let calendar = document.getElementById("calendar");
  calendar.innerHTML = "";

  let date_shows = [];
  for (const episode of episodes) {
    if (date_it == null) {
      date_it = episode.airdate;
    } else if (date_it != episode.airdate) {
      render_date_shows(date_shows, date_it, calendar);
      date_shows = [];
      date_it = episode.airdate;
    }

    const show = shows[episode.show_id];
    date_shows.push(show);
  }
}

init();
