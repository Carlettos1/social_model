# Config
## ver 1
Config {
    version: 1,
    influece_zones_n: 1,
    n_people: people_n,
    n_steps,
    n_burn: 1_000_00,
    runs_per_thread_n: 1,
    n_print: 1000,
    n_log: 1000,
    preparation_n: 1_000_000,
    n_before_flip: n_steps / 6,
    n_flip_accomodation: n_steps / 3,
    q: 2,
    j: 1.5,
    c: 1.0,
    g: 6.0,
    l,
    r,
    ps: 0.5,
    delta1: r,
    delta2: 0.1 * r,
    e_total: -3000.0,
    e_prep: -(((people_n - 1) * 20) as f64),
    e_flip: 4000.0,
}

## ver 2
    e_prep: -((people_n * 20) as f64),

## ver 3
    cambiando a microcanónico

# serialization
en json: 25MB
en binario: 13MB
en f16u8: 1.6MB

problema, sólo un 50% de reducción en tamaño

solución??

## len de vec
Se almacenan como u64, podrían ser u16 (u8 es mucha la constricción, pero 256 > 200)
 * u64 -> u16
 * u64 -> u8

## ppl
son 4 datos
 * internal opinion: u64
 * public opinion: u64
 * pos: (f64, f64)

### opinions
Generalmente es 1 o 0, por lo que se puede guardar en un u8 (o en 1 bit pero leer 1 bit sería molesto)

 * u64 -> u8
 * u64 -> bool

### posición
Se guarda en f64, pero, se puede guardar en f32, o f16 (suficiente presición como para ser graficados)
 * f64 -> f32
 * f64 -> f16

La mezcla de posición (que siempre está en el intervalo 0-1) y un usando la opinión en 1 bit, se puede mezclar para usar el bit de signo de coma flotante como bit de opinión, de tal forma que un f32 o f16 almacenaría toda la información de x-internal opinion, o-public opinion

Por lo que ppl pasaría de 256 bits -> 64 o 32 bits 

## izones
A partir de ahora, sólo se necesitaría izones, los cuales son dependiente de la forma que tenga la zona de influencia. Como generalmente se trabaja con un cuadrado, sólo voy a pensar en ese caso

Izone Cuadrada:
 * variante: u32
 * bottom_left: (f64, f64)
 * top_right: (f64, f64)
 * opinion: u64
 * strenght: f64

La variante de momento es siempre cuadrado así que se puede ignorar

los puntos bottom_left y top_right, pueden moverse a f32 o f16.

Mientras que opinion + strength (siempre positiva) se puede guardar con el mismo truco de posición + opinión

## Resumen

La serialización sería, usando f32 y u16 (antes -> después)
 * vec (64 bits -> 16 bits)
   * len u64 -> u16
 * ppl (256 bits -> 64 bits)
   * io u64 -> u1
   * po u64 -> u1
   * x  f64 -> f31 (f32 sin signo)
   * y  f64 -> f31 (f32 sin signo)
 * izone (416 bits -> 160 bits)
   * va u32 -> None
   * bl (f64, f64) -> (f32, f32)
   * tr (f64, f64) -> (f32, f32)
   * op u64 -> bool
   * st f64 -> f31 (f32 sin signo)

Y, usando f16 y u8 (antes -> después)
 * vec (64 bits -> 8 bits)
   * len u64 -> u8
 * ppl (256 bits -> 32 bits)
   * io u64 -> u1
   * po u64 -> u1
   * x  f64 -> f15 (f16 sin signo)
   * y  f64 -> f15 (f16 sin signo)
 * izone (416 bits -> 80 bits)
   * va u32 -> None
   * bl (f64, f64) -> (f16, f16)
   * tr (f64, f64) -> (f16, f16)
   * op u64 -> bool
   * st f64 -> f15 (f16 sin signo)

Teniendo en cuenta 2 vectores de 200 personas y 1 zona de influencia, los tamaños en binario serían de la forma:

Binario simple: 2 * 64 + 200 * 256 + 1 * 416 = 51744 bits = 6468 bytes
Binario f32u16: 2 * 16 + 200 *  64 + 1 * 160 = 12992 bits = 1624 bytes
Binario f16u8 : 2 *  8 + 200 *  32 + 1 *  80 =  6496 bits =  812 bytes

Usando f16u8 sólo ocupo un 12.55% del espacio de Binario simple, el cual a su vez era un 50% del espacio usando json.

Por lo que, usando f16u8, el archivo en configuración estándar de 25MB, pasaría a <1.6MB (un 6.28% del espacio anterior)

Por su puesto, debido que los vectores no cambian de tamaño, podría quitarse ese dato, y dejarlo, por ejemplo, en el nombre del log (o no ya que está en config/config.log), pero tendría que repensar el graph.py para tomar en cuenta eso (lo dejo a futuro)

También faltaría implementar circle shape al programa, pero se solucionaría usando los bits de signo de las 4 posiciones (u4 para 15 shapes + NoneShape)