package mx.parabellum.juego.fabric.mixin;

import mx.parabellum.juego.fabric.ParabellumFabric;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Options;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Arrays;

/** La tecla de Parabellum en Controles (como lo hace Fabric API, sin depender de ella). */
@Mixin(Options.class)
public class OptionsMixin {
    @Shadow
    @Final
    @Mutable
    public KeyMapping[] keyMappings;

    @Inject(method = "load", at = @At("HEAD"))
    private void parabellum$tecla(CallbackInfo ci) {
        for (KeyMapping k : keyMappings) if (k == ParabellumFabric.ABRIR) return;
        KeyMapping[] todas = Arrays.copyOf(keyMappings, keyMappings.length + 1);
        todas[keyMappings.length] = ParabellumFabric.ABRIR;
        keyMappings = todas;
    }
}
