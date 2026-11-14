package mx.parabellum.juego.neoforge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.properties.Property;
import mx.parabellum.juego.Juego;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.resources.PlayerSkin;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;
import java.util.regex.Pattern;

/** Usa la textura firmada de Parabellum cuando el cliente recibe un perfil de un servidor dedicado. */
public final class SkinsRemotas {
    private record Entry(long expires, CompletableFuture<PlayerSkin> texture) {}
    private static final Pattern NAME = Pattern.compile("[A-Za-z0-9_]{1,16}");
    private static final Map<UUID, Entry> ENTRIES = new ConcurrentHashMap<>();
    private static ClientPacketListener connection;
    private static long next = System.nanoTime();
    private static boolean pending;

    private SkinsRemotas() {}

    static void tick() {
        Minecraft mc = Minecraft.getInstance();
        ClientPacketListener current = mc.isLocalServer() ? null : mc.getConnection();
        long now = System.nanoTime();
        if (current != connection) {
            connection = current;
            ENTRIES.clear();
            next = now;
            pending = false;
        }
        Juego juego = Juego.actual();
        if (current == null || juego == null || pending || now - next < 0) return;
        next = now + TimeUnit.SECONDS.toNanos(3);
        ENTRIES.keySet().removeIf(uuid -> current.getPlayerInfo(uuid) == null);
        var profiles = new LinkedHashMap<UUID, String>();
        JsonArray names = new JsonArray();
        for (PlayerInfo info : current.getOnlinePlayers()) {
            GameProfile player = info.getProfile();
            Entry entry = ENTRIES.get(player.getId());
            if (entry != null && now - entry.expires() < 0) continue;
            if (!NAME.matcher(player.getName()).matches()) continue;
            profiles.put(player.getId(), player.getName());
            names.add(player.getName());
            if (profiles.size() == 24) break;
        }
        if (!profiles.isEmpty()) {
            pending = juego.pedirSkins(names, data -> received(current, profiles, data));
        }
    }

    private static void received(ClientPacketListener request, Map<UUID, String> profiles, JsonObject data) {
        if (request != connection) return;
        pending = false;
        if (data == null) return;
        long checked = System.nanoTime();
        for (var player : profiles.entrySet()) {
            UUID id = player.getKey();
            String name = player.getValue();
            var texture = data.get(name.toLowerCase(Locale.ROOT));
            if (connection.getPlayerInfo(id) == null) continue;
            if (texture == null || !texture.isJsonObject()) {
                ENTRIES.put(id, new Entry(checked + TimeUnit.SECONDS.toNanos(22), null));
                continue;
            }
            JsonObject property = texture.getAsJsonObject();
            GameProfile cosmetic = new GameProfile(id, name);
            cosmetic.getProperties().put("textures", new Property("textures",
                property.get("value").getAsString(), property.get("signature").getAsString()));
            var skin = Minecraft.getInstance().getSkinManager().getOrLoad(cosmetic).exceptionally(error -> null);
            ENTRIES.put(id, new Entry(checked + TimeUnit.SECONDS.toNanos(62), skin));
        }
    }

    public static PlayerSkin forPlayer(PlayerInfo player, PlayerSkin original) {
        Entry entry = ENTRIES.get(player.getProfile().getId());
        if (entry == null || entry.texture() == null) return null;
        PlayerSkin skin = entry.texture().getNow(null);
        if (skin == null || !skin.secure() || skin.textureUrl() == null) return null;
        // La apariencia no cambia la identidad del servidor, la capa, el élitro ni las claves del chat.
        return new PlayerSkin(skin.texture(), skin.textureUrl(), original.capeTexture(), original.elytraTexture(), skin.model(), original.secure());
    }
}
