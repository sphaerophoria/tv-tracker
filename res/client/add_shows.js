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
  return document.getElementById("show-search-button");
}

function get_search_box() {
  return document.getElementById("show-search");
}

function handle_search_keypress(event) {
  if (event.key === "Enter") {
    event.preventDefault();
    get_search_button().click();
  }
}

async function handle_add(item) {
  const request = new Request("shows", {
    method: "PUT",
    body: JSON.stringify({
      remote_id: item,
    }),
  });
  fetch(request);
}

function render_show(item, parent) {
  const href = item.url;
  const card = render_card_element(item, href);

  const add_button = document.createElement("input");
  add_button.type = "button";
  add_button.value = "+";
  add_button.classList.add("card-add-button");
  add_button.onclick = () => handle_add(item.id);
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

  const search_results = document.getElementById("search-results");

  for (const i in response_body) {
    const item = response_body[i];
    render_show(item, search_results);
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
