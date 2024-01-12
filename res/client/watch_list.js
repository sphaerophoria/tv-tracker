"use strict";

import { get_movies, get_shows, get_ratings } from "./http.js";
import { render_card_element } from "./show_card.js";

function sort_items_by_name(items) {
  items.sort((a, b) => {
    return a.item.name.toLowerCase() > b.item.name.toLowerCase() ? 1 : -1;
  });
}

function render_items(items, parent) {
  sort_items_by_name(items);

  let ret = "";
  for (const item of items) {
    const card = render_card_element(item.item, item.href);
    parent.appendChild(card);
  }
  return ret;
}

async function render_by_group(groups, div, page_mode) {
  for (const group of groups) {
    if (group.items.length == 0) {
      continue;
    }

    const header = document.createElement("h1");
    header.innerText = group.name;
    div.appendChild(header);

    const links = document.createElement("div");
    links.classList.add("show-list");
    render_items(group.items, links);
    div.appendChild(links);
  }
}

function group_by_rating(shows_obj, ratings_obj) {
  let shows = Object.values(shows_obj);
  let ratings = Object.values(ratings_obj).sort(
    (a, b) => a.priority - b.priority,
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

function group_shows_by_watch_status(shows) {
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

function group_movies_by_watch_status(movies) {
  let unwatched = [];
  let in_theaters = [];
  let unreleased = [];
  let watched = [];

  let today = new Date(Date.now());
  for (const movie of Object.values(movies)) {
    if (movie.watched) {
      watched.push(movie);
    } else if (
      movie.theater_release_date === null ||
      today < new Date(movie.theater_release_date)
    ) {
      unreleased.push(movie);
    } else if (
      movie.home_release_date === null ||
      today < new Date(movie.home_release_date)
    ) {
      in_theaters.push(movie);
    } else {
      unwatched.push(movie);
    }
  }

  let groups = [
    {
      name: "Unwatched",
      items: unwatched,
    },
    {
      name: "In Theaters",
      items: in_theaters,
    },
    {
      name: "Unreleased",
      items: unreleased,
    },
    {
      name: "Watched",
      items: watched,
    },
  ];

  return groups;
}

function group_items_by_watch_status(watch_items, page_mode) {
  if (page_mode == PageMode.SHOWS) {
    return group_shows_by_watch_status(watch_items);
  } else if (page_mode == PageMode.MOVIES) {
    return group_movies_by_watch_status(watch_items);
  } else {
    throw new Error("Invalid page mode");
  }
}

class WatchItemPage {
  constructor(watch_items, ratings, page_mode) {
    this.watch_items = watch_items;
    this.ratings = ratings;
    this.page_mode = page_mode;
  }

  render() {
    let sort_style_selector = document.getElementById("sort-style");
    let sort_style = sort_style_selector.selectedIndex;

    let grouped_shows = [];
    if (sort_style == 0) {
      grouped_shows = group_items_by_watch_status(
        this.watch_items,
        this.page_mode,
      );
    } else if (sort_style == 1) {
      grouped_shows = group_by_rating(this.watch_items, this.ratings);
    } else if (sort_style == 2) {
      grouped_shows = [
        {
          name: page_mode_to_string(this.page_mode),
          items: Object.values(this.watch_items),
        },
      ];
    }

    // Here we have items as an array of watch items, but we need to encode their hrefs depending on if they're movies or shows
    let item_with_href = null;
    if (this.page_mode == PageMode.SHOWS) {
      item_with_href = (item) => {
        return {
          item: item,
          href: "show.html?show_id=" + item.id,
        };
      };
    } else if (this.page_mode == PageMode.MOVIES) {
      item_with_href = (item) => {
        return {
          item: item,
          href: "movie.html?movie_id=" + item.id,
        };
      };
    }

    for (const group of grouped_shows) {
      group.items = group.items.map(item_with_href);
    }

    let shows_div = document.getElementById("shows");
    shows_div.innerHTML = "";
    render_by_group(grouped_shows, shows_div, this.page_mode);
  }
}

const PageMode = {
  MOVIES: 0,
  SHOWS: 1,
};

function get_page_mode() {
  const my_url = new URL(document.URL);
  if (my_url.searchParams.has("movies")) {
    return PageMode.MOVIES;
  } else {
    return PageMode.SHOWS;
  }
}

function page_mode_to_string(page_mode) {
  if (page_mode == PageMode.MOVIES) {
    return "Movies";
  } else if (page_mode == PageMode.SHOWS) {
    return "Shows";
  } else {
    throw new Error("Invalid page mode");
  }
}

async function init() {
  const page_mode = get_page_mode();

  let watch_items_promise = null;
  if (page_mode == PageMode.MOVIES) {
    watch_items_promise = get_movies();
  } else {
    watch_items_promise = get_shows();
  }

  const ratings_promise = get_ratings();

  let watch_items, ratings;
  [watch_items, ratings] = await Promise.all([
    watch_items_promise,
    ratings_promise,
  ]);
  const show_page = new WatchItemPage(watch_items, ratings, page_mode);
  show_page.render();

  let sort_style_selector = document.getElementById("sort-style");
  sort_style_selector.onchange = () => {
    show_page.render();
  };
}

init();
