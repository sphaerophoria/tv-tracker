"use strict";

import { render_card_element } from "./show_card.js";

function handle_search(element) {
  if (event.key === "Enter") {
    const request = new Request("/search?" + element.value, {
      method: "GET",
    });
    fetch(request);
  }
}

function get_search_button() {
  return document.getElementById("search-button");
}

function get_search_box() {
  return document.getElementById("search-box");
}

function handle_search_keypress(event) {
  if (event.key === "Enter") {
    event.preventDefault();
    get_search_button().click();
  }
}

async function handle_add_show(item) {
  const request = new Request("shows", {
    method: "PUT",
    body: JSON.stringify({
      remote_id: item,
    }),
  });
  fetch(request);
}

async function handle_add_movie(item) {
  const request = new Request("movies", {
    method: "PUT",
    body: JSON.stringify({
      imdb_id: item,
    }),
  });
  fetch(request);
}

function render_watch_item(item, parent, onclick) {
  let href = null;
  if (item.imdb_id !== null && item.imdb_id !== undefined) {
    href = "https://www.imdb.com/title/" + item.imdb_id;
  } else if (item.url !== undefined) {
    href = item.url;
  }

  const card = render_card_element(item, href);

  const add_button = document.createElement("input");
  add_button.type = "button";
  add_button.value = "+";
  add_button.classList.add("card-add-button");
  add_button.onclick = onclick;
  card.appendChild(add_button);

  parent.appendChild(card);
}

async function execute_search() {
  const params = new URLSearchParams({ query: get_search_box().value });
  const request = new Request("search?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  const response_body = await response.json();

  const show_search_results = document.getElementById("show-search-results");
  show_search_results.innerHTML = "";

  for (const i in response_body.shows) {
    const item = response_body.shows[i];
    render_watch_item(item, show_search_results, () =>
      handle_add_show(item.id),
    );
  }

  const movie_search_results = document.getElementById("movie-search-results");
  movie_search_results.innerHTML = "";
  for (const i in response_body.movies) {
    const item = response_body.movies[i];
    render_watch_item(item, movie_search_results, () =>
      handle_add_movie(item.imdb_id),
    );
  }
}

function setup_search_handlers() {
  const search_box = get_search_box();
  search_box.addEventListener("keypress", handle_search_keypress);
  get_search_button().onclick = execute_search;
}

async function init() {
  setup_search_handlers();
}

init();
