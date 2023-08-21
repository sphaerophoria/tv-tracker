"use strict";

import { get_shows, get_ratings } from "./http.js";
import { render_card_element } from "./show_card.js";

function sort_shows_by_name(shows) {
  shows.sort((a, b) => a.name.toLowerCase() > b.name.toLowerCase());
}

function render_shows(shows, parent) {
  sort_shows_by_name(shows);

  let ret = "";
  for (const show of shows) {
    const href = "/show.html?show_id=" + show.id;

    const card = render_card_element(show, href);
    parent.appendChild(card);
  }
  return ret;
}

async function render_by_group(groups, div) {
  for (const group of groups) {
    const header = document.createElement("h1");
    header.innerText = group.name;
    div.appendChild(header);

    const links = document.createElement("div");
    links.classList.add("show-list");
    render_shows(group.items, links);
    div.appendChild(links);
  }
}

function group_by_rating(shows_obj, ratings_obj) {
  let shows = Object.values(shows_obj);
  let ratings = Object.values(ratings_obj).sort(
    (a, b) => a.priority >= b.priority
  );
  let groups = [];

  for (const rating of ratings) {
    const rating_shows = shows.filter((elem) => elem.rating_id == rating.id);
    groups.push({
      name: rating.name,
      items: rating_shows,
    });
  }

  const unrated_shows = shows.filter((elem) => elem.rating_id == null);
  groups.push({
    name: "Unrated",
    items: unrated_shows,
  });
  return groups;
}

function group_by_watch_status(shows) {
  let next_up_shows = [];
  let finished_shows = [];
  let unstarted_shows = [];
  let paused_shows = [];

  for (const show of Object.values(shows)) {
    if (show.episodes_watched == show.episodes_aired) {
      finished_shows.push(show);
    } else if (show.pause_status) {
      paused_shows.push(show);
    } else if (show.episodes_watched > 0) {
      next_up_shows.push(show);
    } else {
      unstarted_shows.push(show);
    }
  }

  let groups = [
    {
      name: "Next up",
      items: next_up_shows,
    },
    {
      name: "Unstarted",
      items: unstarted_shows,
    },
    {
      name: "Paused",
      items: paused_shows,
    },
    {
      name: "Finished/Caught up",
      items: finished_shows,
    },
  ];

  return groups;
}

class ShowPage {
  constructor(shows, ratings) {
    this.shows = shows;
    this.ratings = ratings;
  }

  render() {
    let sort_style_selector = document.getElementById("sort-style");
    let sort_style = sort_style_selector.selectedIndex;

    let grouped_shows = [];
    if (sort_style == 0) {
      grouped_shows = group_by_watch_status(this.shows);
    } else if (sort_style == 1) {
      grouped_shows = group_by_rating(this.shows, this.ratings);
    } else if (sort_style == 2) {
      grouped_shows = [
        {
          name: "Shows",
          items: Object.values(this.shows),
        },
      ];
    }

    let shows_div = document.getElementById("shows");
    shows_div.innerHTML = "";
    render_by_group(grouped_shows, shows_div);
  }
}

async function init() {
  const shows_promise = get_shows();
  const ratings_promise = get_ratings();

  let shows, ratings;
  [shows, ratings] = await Promise.all([shows_promise, ratings_promise]);
  const show_page = new ShowPage(shows, ratings);
  show_page.render();

  let sort_style_selector = document.getElementById("sort-style");
  sort_style_selector.onchange = () => {
    show_page.render();
  };
}

init();
