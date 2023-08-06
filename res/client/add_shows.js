"use strict";

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
  const request = new Request("/add_show", {
    method: "PUT",
    body: JSON.stringify({
      id: item,
    }),
  });
  fetch(request);
}

function render_show(item) {
  let rendered = '<div class="show-card">';

  if (item.show.url !== null) {
    rendered += "<a href=" + item.show.url + ">";
  }

  if (item.show.image !== null) {
    rendered += '<img class="card-image" src=' + item.show.image + ">";
  } else {
    rendered += '<div class="card-placeholder-image"></div>';
  }

  if (item.show.url !== null) {
    rendered += "</a>";
  }

  rendered +=
    '<input onclick="handle_add(' + item.id + ')" type="button" value="+"/>';
  rendered += "</div>";

  return rendered;
}

async function execute_search() {
  const params = new URLSearchParams({ query: get_search_box().value });
  const request = new Request("/search?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  const response_body = await response.json();

  const search_results = document.getElementById("search-results");

  let new_html = "";

  for (const i in response_body) {
    const item = response_body[i];
    new_html += render_show(item);
  }

  search_results.innerHTML = new_html;
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
