async function get_json(url) {
  const request = new Request(url, {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

async function put_json(url, data) {
  const request = new Request(url,{
    method: "PUT",
    body: JSON.stringify(data),
  });
  const response = await fetch(request);
  return await response.json();
}

export async function get_show_episodes(show_id) {
  return await get_json("shows/" + show_id + "/episodes")
}

export async function get_shows() {
  return await get_json("shows")
}

export async function get_show(show_id) {
  return await get_json("shows/" + show_id)
}

export async function put_show(show) {
  return await put_json("shows/" + show.id, show)
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
  return await get_json("episodes?" + params.toString())
}

export async function put_episode(episode) {
  return await put_json("episodes/" + episode.id, episode)
}

export async function get_ratings() {
  return await get_json("ratings")
}

export async function put_ratings(rating) {
  return await put_json("ratings", rating)
}

export async function put_rating(rating) {
  return await put_json("ratings/" + rating.id, rating);
}

export async function delete_rating(rating_id) {
  const request = new Request("ratings/" + rating_id, {
    method: "DELETE",
  });
  const response = await fetch(request);
}

export async function get_movies() {
  return await get_json("movies")
}

export async function get_tv_sprite_sheet_info() {
  return await get_json("tv_sprite_sheet_info")
}

export async function get_movie(movie_id) {
  return await get_json("movies/" + movie_id)
}

export async function put_movie(movie) {
  return await put_json("movies/" + movie.id, movie);
}

export async function delete_movie(movie_id) {
  const request = new Request("movies/" + movie_id, {
    method: "DELETE",
  });
  const response = await fetch(request);
}
