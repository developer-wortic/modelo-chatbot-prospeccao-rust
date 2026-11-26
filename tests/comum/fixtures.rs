//! Respostas de streaming dos provedores, como chegam pelo fio.
//! `OpenAI` e `Anthropic`: linhas copiadas da documentação oficial (exemplos de streaming).
//! `OpenRouter`: formato do exemplo de chunk da documentação. `Groq` e `Gemini`: formato do esquema da documentação.
//! Cada linha de dados JSON leva o comentário `borda-json` com o provedor de origem.

/// Streaming da Responses API ), com o texto "Hi there!" em pedaços.
pub const OPENAI_SSE: &[&str] = &[
    "event: response.created\n",
    "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_67c9fdcecf488190bdd9a0409de3a1ec07b8b0ad4e5eb654\",\"object\":\"response\",\"status\":\"in_progress\",\"model\":\"gpt-6-astra\",\"output\":[]},\"sequence_number\":0}\n\n", // borda-json: fio da `OpenAI` (terceiro)
    "event: response.output_text.delta\n",
    "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg_67c9fdcf37fc8190ba82116e33fb28c507b8b0ad4e5eb654\",\"output_index\":0,\"content_index\":0,\"delta\":\"Hi\",\"sequence_number\":4,\"logprobs\":[]}\n\n", // borda-json: fio da `OpenAI` (terceiro)
    ": Intermediate response events omitted.\n\n",
    "event: response.output_text.delta\n",
    "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg_67c9fdcf37fc8190ba82116e33fb28c507b8b0ad4e5eb654\",\"output_index\":0,\"content_index\":0,\"delta\":\" there!\",\"sequence_number\":5,\"logprobs\":[]}\n\n", // borda-json: fio da `OpenAI` (terceiro)
    "event: response.output_text.done\n",
    "data: {\"type\":\"response.output_text.done\",\"item_id\":\"msg_67c9fdcf37fc8190ba82116e33fb28c507b8b0ad4e5eb654\",\"output_index\":0,\"content_index\":0,\"text\":\"Hi there!\",\"sequence_number\":10,\"logprobs\":[]}\n\n", // borda-json: fio da `OpenAI` (terceiro)
    "event: response.completed\n",
    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_67c9fdcecf488190bdd9a0409de3a1ec07b8b0ad4e5eb654\",\"object\":\"response\",\"status\":\"completed\",\"model\":\"gpt-6-astra\",\"output\":[]},\"sequence_number\":13}\n\n", // borda-json: fio da `OpenAI` (terceiro)
];

/// Streaming da Messages API (), copiado do exemplo de resposta da documentação, com ping.
pub const ANTHROPIC_SSE: &[&str] = &[
    "event: message_start\n",
    "data: {\"type\": \"message_start\", \"message\": {\"id\": \"msg_1nZdL29xx5MUA1yADyHTEsnR8uuvGzszyY\", \"type\": \"message\", \"role\": \"assistant\", \"content\": [], \"model\": \"claude-haiku-5-5\", \"stop_reason\": null, \"stop_sequence\": null, \"usage\": {\"input_tokens\": 25, \"output_tokens\": 1}}}\n\n", // borda-json: fio da `` (terceiro)
    "event: content_block_start\n",
    "data: {\"type\": \"content_block_start\", \"index\": 0, \"content_block\": {\"type\": \"text\", \"text\": \"\"}}\n\n", // borda-json: fio da `` (terceiro)
    "event: ping\n",
    "data: {\"type\": \"ping\"}\n\n", // borda-json: fio da `` (terceiro)
    "event: content_block_delta\n",
    "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"Hello\"}}\n\n", // borda-json: fio da `` (terceiro)
    "event: content_block_delta\n",
    "data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"!\"}}\n\n", // borda-json: fio da `` (terceiro)
    "event: content_block_stop\n",
    "data: {\"type\": \"content_block_stop\", \"index\": 0}\n\n", // borda-json: fio da `` (terceiro)
    "event: message_delta\n",
    "data: {\"type\": \"message_delta\", \"delta\": {\"stop_reason\": \"end_turn\", \"stop_sequence\":null}, \"usage\": {\"output_tokens\": 15}}\n\n", // borda-json: fio da `` (terceiro)
    "event: message_stop\n",
    "data: {\"type\": \"message_stop\"}\n\n", // borda-json: fio da `` (terceiro)
];

/// Streaming do `Gemini` (`alt=sse`): cada evento é um `GenerateContentResponse` do esquema da documentação.
pub const GEMINI_SSE: &[&str] = &[
    "data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \"Olá\"}], \"role\": \"model\"}, \"index\": 0}]}\n\n", // borda-json: fio do `Gemini` (terceiro)
    "data: {\"candidates\": [{\"content\": {\"parts\": [{\"text\": \", tudo bem?\"}], \"role\": \"model\"}, \"finishReason\": \"STOP\", \"index\": 0}]}\n\n", // borda-json: fio do `Gemini` (terceiro)
];

/// Streaming de chat completions ( e OpenRouter): chunks `chat.completion.chunk`, terminando em `[DONE]`.
pub const CHAT_SSE: &[&str] = &[
    ": OPENROUTER PROCESSING\n\n",
    "data: {\"id\":\"gen-abc123\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"modelo\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Olá\"},\"finish_reason\":null}]}\n\n", // borda-json: fio de chat completions (terceiro)
    "data: {\"id\":\"gen-abc123\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"modelo\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" tudo bem?\"},\"finish_reason\":null}]}\n\n", // borda-json: fio de chat completions (terceiro)
    "data: {\"id\":\"gen-abc123\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"modelo\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\",\"role\":\"assistant\"},\"finish_reason\":\"stop\"}]}\n\n", // borda-json: fio de chat completions (terceiro)
    "data: [DONE]\n\n",
];
