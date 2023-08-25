export async function get_show_episodes(show_id) {
  const request = new Request("shows/" + show_id + "/episodes", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function get_shows() {
  const request = new Request("shows", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function get_show(show_id) {
  const request = new Request("shows/" + show_id, {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function put_show(show) {
  const request = new Request("shows/" + show.id, {
    method: "PUT",
    body: JSON.stringify(show),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function delete_show(show_id) {
  const request = new Request("shows/" + show_id, {
    method: "DELETE",
  });
  return await fetch(request);
}

export async function get_episodes(start_date, end_date) {
  const params = new URLSearchParams({
    start_date: start_date,
    end_date: end_date,
  });
  const request = new Request("episodes?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function put_episode(episode) {
  const request = new Request("episodes/" + episode.id, {
    method: "PUT",
    body: JSON.stringify(episode),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function get_ratings() {
  const request = new Request("ratings", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function put_ratings(rating) {
  const request = new Request("ratings", {
    method: "PUT",
    body: JSON.stringify(rating),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function put_rating(rating) {
  const request = new Request("ratings/" + rating.id, {
    method: "PUT",
    body: JSON.stringify(rating),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function delete_rating(rating_id) {
  const request = new Request("ratings/" + rating_id, {
    method: "DELETE",
  });
  const response = await fetch(request);
}

export async function get_movies() {
  const request = new Request("movies", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function get_movie(movie_id) {
  const request = new Request("movies/" + movie_id, {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function put_movie(movie) {
  const request = new Request("movies/" + movie.id, {
    method: "PUT",
    body: JSON.stringify(movie),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function delete_movie(movie_id) {
  const request = new Request("movies/" + movie_id, {
    method: "DELETE",
  });
  const response = await fetch(request);
}
