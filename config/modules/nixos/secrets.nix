{
  username,
  ...
}:
{
  age.secrets."openai-api-key" = {
    file = ../../secrets/openai-api-key.age;
    owner = username;
    group = "users";
    mode = "0400";
  };
}
