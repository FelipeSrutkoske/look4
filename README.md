# Look4

> **Verificador rápido de links com a API do VirusTotal para GNOME / Linux.**

---

## Requisitos do Sistema

Certifique-se de ter as bibliotecas de desenvolvimento do GTK4 e do Libadwaita instaladas:

### Ubuntu / Debian / Pop!_OS:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev libssl-dev libdbus-1-dev
```

### Fedora:
```bash
sudo dnf install -y gcc pkgconf-pkg-config gtk4-devel libadwaita-devel openssl-devel dbus-devel
```

### Arch Linux:
```bash
sudo pacman -S --needed base-devel gtk4 libadwaita openssl dbus
```

---

## Instalação Rápida

1. **Clone o repositório:**
   ```bash
   git clone https://github.com/FelipeSrutkoske/look4.git
   cd look4
   ```

2. **Compile e Instale no Usuário:**
   ```bash
   cargo build --release
   ./scripts/install-user.sh
   ```

O instalador irá:
- Instalar o binário em `~/.local/bin/look4`.
- Criar o lançador no menu de aplicativos do GNOME.
- Ativar o autostart na inicialização do sistema.
- Configurar o atalho global <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>V</kbd>.

3. **Inicie o aplicativo:**
   Abra pelo menu de aplicativos ou execute:
   ```bash
   look4 &
   ```

---

## Configuracao da Chave de API (VirusTotal)

1. Obtenha sua chave gratuita em [virustotal.com/gui/my-apikey](https://www.virustotal.com/gui/my-apikey).
2. No Look4, clique no ícone de engrenagem (canto superior direito).
3. Cole a chave de API e clique em **Salvar chave**.
4. Ela ficará armazenada no seu chaveiro criptografado do sistema.

---


## Desinstalação

Caso queira remover o aplicativo e os atalhos do sistema:

```bash
./scripts/uninstall-user.sh
rm -f ~/.local/bin/look4
```

---

## Licença

Distribuído sob a licença **MIT**. Veja `LICENSE` para mais detalhes.
