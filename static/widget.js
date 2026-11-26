/* Widget do chatbot de prospecção. Um script só, sem dependência. Cole no site com:
   <script src="https://SEU-SERVIDOR/widget.js" defer></script>
   O endereço da API é o próprio endereço de onde o script veio. Texto sempre com textContent (sem HTML). */
(function () {
  "use strict";
  var atual = document.currentScript;
  var base = atual && atual.src ? new URL(atual.src).origin : "";
  var sessao = null;
  var ocupado = false;

  var css = [
    ".wc-botao{position:fixed;right:20px;bottom:20px;z-index:2147483000;height:52px;padding:0 20px;border:0;border-radius:26px;background:#0071e3;color:#fff;font:600 15px/1 -apple-system,BlinkMacSystemFont,Arial,sans-serif;box-shadow:0 6px 20px rgba(0,0,0,.2);cursor:pointer}",
    ".wc-painel{position:fixed;right:20px;bottom:84px;z-index:2147483000;width:340px;max-width:calc(100vw - 40px);height:460px;max-height:calc(100vh - 110px);display:flex;flex-direction:column;background:#fff;border:1px solid #e8e8ed;border-radius:18px;box-shadow:0 12px 40px rgba(0,0,0,.18);font:15px/1.4 -apple-system,BlinkMacSystemFont,Arial,sans-serif;color:#1d1d1f;overflow:hidden}",
    ".wc-cabeca{display:flex;justify-content:space-between;align-items:center;padding:14px 16px;border-bottom:1px solid #e8e8ed;font-weight:600}",
    ".wc-cabeca button{border:0;background:none;color:#0071e3;font:inherit;cursor:pointer;padding:0}",
    ".wc-msgs{flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:8px}",
    ".wc-msg{max-width:84%;padding:9px 12px;border-radius:14px;white-space:pre-wrap;word-break:break-word}",
    ".wc-eu{align-self:flex-end;background:#0071e3;color:#fff;border-bottom-right-radius:4px}",
    ".wc-bot{align-self:flex-start;background:#f5f5f7;border-bottom-left-radius:4px}",
    ".wc-form{display:flex;gap:8px;padding:12px;border-top:1px solid #e8e8ed}",
    ".wc-form input{flex:1;min-width:0;height:42px;padding:0 12px;border:1px solid #d2d2d7;border-radius:10px;font:inherit}",
    ".wc-form button{height:42px;padding:0 14px;border:0;border-radius:10px;background:#0071e3;color:#fff;font:inherit;cursor:pointer}",
    ".wc-form button:disabled{opacity:.5}",
    ".wc-fim{font-size:13px;color:#6e6e73;padding:0 14px 12px;text-align:center}"
  ].join("");

  var estilo = document.createElement("style");
  estilo.textContent = css;
  document.head.appendChild(estilo);

  var botao = document.createElement("button");
  botao.className = "wc-botao";
  botao.type = "button";
  botao.textContent = "Fale com a gente";
  document.body.appendChild(botao);

  var painel = document.createElement("div");
  painel.className = "wc-painel";
  painel.hidden = true;
  painel.innerHTML = "";
  var cabeca = document.createElement("div");
  cabeca.className = "wc-cabeca";
  var titulo = document.createElement("span");
  titulo.textContent = "Atendimento";
  var fechar = document.createElement("button");
  fechar.type = "button";
  fechar.textContent = "Fechar";
  cabeca.appendChild(titulo);
  cabeca.appendChild(fechar);
  var msgs = document.createElement("div");
  msgs.className = "wc-msgs";
  var form = document.createElement("form");
  form.className = "wc-form";
  var campo = document.createElement("input");
  campo.placeholder = "Escreva sua mensagem";
  campo.maxLength = 1000;
  var enviar = document.createElement("button");
  enviar.type = "submit";
  enviar.textContent = "Enviar";
  form.appendChild(campo);
  form.appendChild(enviar);
  var rodape = document.createElement("div");
  rodape.className = "wc-fim";
  var encerrar = document.createElement("button");
  encerrar.type = "button";
  encerrar.textContent = "Terminar conversa";
  encerrar.style.cssText = "border:0;background:none;color:#0071e3;font:inherit;cursor:pointer";
  rodape.appendChild(encerrar);
  painel.appendChild(cabeca);
  painel.appendChild(msgs);
  painel.appendChild(form);
  painel.appendChild(rodape);
  document.body.appendChild(painel);

  function linha(texto, quem) {
    var bolha = document.createElement("div");
    bolha.className = "wc-msg " + (quem === "eu" ? "wc-eu" : "wc-bot");
    bolha.textContent = texto;
    msgs.appendChild(bolha);
    msgs.scrollTop = msgs.scrollHeight;
  }

  function pedir(caminho, corpo) {
    return fetch(base + caminho, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(corpo) // borda-json: corpo enviado ao chat do próprio servidor
    }).then(function (r) {
      return r.json().then(function (dados) { // borda-json: resposta do chat
        return { ok: r.ok, dados: dados };
      });
    });
  }

  function erroDoServidor(dados) {
    return (dados && dados.mensagem) || "Não consegui responder agora. Tente de novo em instantes.";
  }

  function mandar(evento) {
    evento.preventDefault();
    var texto = campo.value.trim();
    if (!texto || ocupado) return;
    ocupado = true;
    enviar.disabled = true;
    linha(texto, "eu");
    campo.value = "";
    pedir("/api/chat", { sessao: sessao, mensagem: texto })
      .then(function (r) {
        if (r.ok) {
          sessao = r.dados.sessao;
          linha(r.dados.resposta, "bot");
        } else {
          linha(erroDoServidor(r.dados), "bot");
        }
      })
      .catch(function () { linha("Sem conexão com o atendimento agora.", "bot"); })
      .then(function () { ocupado = false; enviar.disabled = false; campo.focus(); });
  }

  function terminar() {
    if (!sessao || ocupado) {
      linha("Ainda não há conversa para terminar.", "bot");
      return;
    }
    ocupado = true;
    pedir("/api/encerrar", { sessao: sessao })
      .then(function (r) {
        linha(r.ok ? "Obrigado! Sua conversa foi registrada e a equipe vai falar com você." : erroDoServidor(r.dados), "bot");
        if (r.ok) { sessao = null; }
      })
      .catch(function () { linha("Sem conexão agora. Tente de novo.", "bot"); })
      .then(function () { ocupado = false; });
  }

  botao.addEventListener("click", function () {
    painel.hidden = !painel.hidden;
    if (!painel.hidden && msgs.childNodes.length === 0) {
      linha("Olá! Sou o assistente virtual. Em que posso ajudar?", "bot");
    }
    if (!painel.hidden) campo.focus();
  });
  fechar.addEventListener("click", function () { painel.hidden = true; });
  form.addEventListener("submit", mandar);
  encerrar.addEventListener("click", terminar);
})();
