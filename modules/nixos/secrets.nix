{
  username,
  ...
}:
{
  age.secrets.hindsight-client-token = {
    file = ../../secrets/hindsight-client-token.age;
    owner = username;
    group = "users";
    mode = "0400";
  };
  age.secrets."openai-api-key" = {
    file = ../../secrets/openai-api-key.age;
    owner = username;
    group = "users";
    mode = "0400";
  };
}
