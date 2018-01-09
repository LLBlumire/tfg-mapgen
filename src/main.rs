#[macro_use]
extern crate serde_derive;

extern crate image;
extern crate rand;
extern crate toml;

use rand::Rng;
use rand::thread_rng;
use std::fs::File;
use std::env::args;
use std::io::Read;
use image::Rgb;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Deref;
use image::ImageBuffer;

type Color = Rgb<u8>;
type RcMut<T> = Rc<RefCell<T>>;

fn false_v() -> bool {
    false
}

fn d20<R: Rng>(r: &mut R) -> usize {
    r.gen_range(1, 21)
}

fn d4<R: Rng>(r: &mut R) -> usize {
    r.gen_range(1, 5)
}

fn d4_to_dx(n: usize) -> isize {
    match n {
        1 => 0,
        2 => 1,
        3 => 0,
        4 => -1,
        _ => panic!(),
    }
}

fn d4_to_dy(n: usize) -> isize {
    match n {
        1 => 1,
        2 => 0,
        3 => -1,
        4 => 0,
        _ => panic!(),
    }
}

fn hex_to_color(n: &str) -> Color {
    let n = &n[1..];
    Color {
        data: [
            u8::from_str_radix(&n[0..2], 16).unwrap(),
            u8::from_str_radix(&n[2..4], 16).unwrap(),
            u8::from_str_radix(&n[4..6], 16).unwrap(),
        ],
    }
}

fn linear_index(x: usize, y: usize, xsize: usize) -> usize {
    (y * xsize) + x
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Input {
    #[serde(rename = "tile")]
    tiles: Vec<TileInput>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TileInput {
    name: String,
    color: String,
    inner_color: Option<String>,
    #[serde(default = "false_v")]
    village: bool,
    #[serde(default = "false_v")]
    tower: bool,
    #[serde(default = "false_v")]
    corruption: bool,
    #[serde(default = "Vec::new")]
    nextgen: Vec<NextGen>,
    limit: usize,
}
impl TileInput {
    fn roll<R: Rng>(
        &self,
        rng: &mut R,
        tiles: &HashMap<String, RcMut<TileInput>>,
        corruption: &RcMut<TileInput>,
    ) -> RcMut<TileInput> {
        let mut r = d20(rng);
        let mut target: RcMut<TileInput> = Rc::clone(&corruption);
        loop {
            if r == 1 || r == 20 {
                target = Rc::clone(rng.choose(&tiles.values().collect::<Vec<_>>()).unwrap());
            } else {
                for gen in self.nextgen.iter() {
                    if r >= gen.lower && r <= gen.upper {
                        target = Rc::clone(tiles.get(&gen.name).unwrap());
                    }
                }
            }

            if target.borrow().limit != 0 {
                break;
            }
            r -= 1;
            if r <= 1 {
                target = Rc::clone(&corruption);
                break;
            }
        }

        return target;
    }
}

#[derive(Debug, Clone)]
struct Tile {
    id: String,
    color: Color,
    inner_color: Color,
}
impl From<TileInput> for Tile {
    fn from(input: TileInput) -> Tile {
        let color = hex_to_color(&input.color);
        let inner_color;
        if let &Some(ref inner) = &input.inner_color {
            inner_color = hex_to_color(inner);
        } else {
            inner_color = color.clone();
        }
        Tile {
            id: input.name,
            color: color,
            inner_color: inner_color,
        }
    }
}
impl<'a> From<&'a TileInput> for Tile {
    fn from(input: &TileInput) -> Tile {
        Tile::from(input.clone())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NextGen {
    name: String,
    lower: usize,
    upper: usize,
}

#[derive(Copy, Clone, Debug)]
struct Ant {
    x: usize,
    y: usize,
}
impl Ant {
    fn new(x: usize, y: usize) -> Ant {
        Ant { x, y }
    }

    fn update<R: Rng>(&mut self, rng: &mut R) {
        let mut rd;
        while {
            rd = d4(rng);
            self.x as isize + d4_to_dx(rd) < 1 || self.x as isize + d4_to_dx(rd) > 20 ||
                self.y as isize + d4_to_dy(rd) < 1 ||
                self.y as isize + d4_to_dy(rd) > 20
        }
        {}
        self.x = (self.x as isize + d4_to_dx(rd)) as usize;
        self.y = (self.y as isize + d4_to_dy(rd)) as usize;
    }
}

#[derive(Debug)]
struct Map {
    map: Vec<Option<Tile>>,
}
impl Map {
    fn new() -> Map {
        Map { map: vec![None; 20 * 20] }
    }

    fn get(&self, x: usize, y: usize) -> Option<&Option<Tile>> {
        self.map.get(linear_index(x - 1, y - 1, 20))
    }

    fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut Option<Tile>> {
        self.map.get_mut(linear_index(x - 1, y - 1, 20))
    }

    fn check_put(&mut self, x: usize, y: usize, tile: Tile) -> bool {
        if let Some(n) = self.get_mut(x, y) {
            if n.is_some() {
                return false;
            } else {
                *n = Some(tile);
                return true;
            }
        }
        return false;
    }

    fn is_full(&mut self) -> bool {
        self.map.iter().all(|o| o.is_some())
    }
}

fn main() {
    println!("Initialising");
    let ref mut rng = thread_rng();
    let mut args = args();
    let _ = args.next();
    let file_path = args.next().unwrap();
    let num_villages = args.next().unwrap().trim().parse::<usize>().unwrap();


    let mut file = File::open(file_path).unwrap();
    let mut input = String::new();
    file.read_to_string(&mut input).unwrap();

    let mut map = Map::new();
    let mut input: Input = toml::from_str(&input).unwrap();
    let mut ants: Vec<Ant> = Vec::new();

    let mut tiles: HashMap<String, RcMut<TileInput>> = input
        .tiles
        .into_iter()
        .map(|tile| (tile.name.clone(), Rc::new(RefCell::new(tile))))
        .collect();

    let mut village: RcMut<TileInput> = Rc::clone(
        tiles
            .values()
            .filter(|v| v.borrow().village)
            .next()
            .unwrap(),
    );
    let mut tower: RcMut<TileInput> =
        Rc::clone(tiles.values().filter(|v| v.borrow().tower).next().unwrap());

    let mut corruption: RcMut<TileInput> = Rc::clone(
        tiles
            .values()
            .filter(|v| v.borrow().corruption)
            .next()
            .unwrap(),
    );

    // Place City
    {
        println!("Placing City");
        let mut rx;
        let mut ry;
        while {
            rx = d20(rng);
            ry = d20(rng);
            rx < 2 || rx > 19 || ry < 2 || ry > 19
        }
        {}

        map.check_put(rx + 1, ry + 1, village.borrow().deref().into());
        map.check_put(rx + 1, ry, village.borrow().deref().into());
        map.check_put(rx + 1, ry - 1, village.borrow().deref().into());
        map.check_put(rx, ry + 1, village.borrow().deref().into());
        map.check_put(rx, ry, village.borrow().deref().into());
        map.check_put(rx, ry - 1, village.borrow().deref().into());
        map.check_put(rx - 1, ry + 1, village.borrow().deref().into());
        map.check_put(rx - 1, ry, village.borrow().deref().into());
        map.check_put(rx - 1, ry - 1, village.borrow().deref().into());
    }

    // Place Ants & Starting Villages
    {
        println!("Placing Starting Villages");
        for _ in 0..num_villages {
            let mut rx;
            let mut ry;
            while {
                rx = d20(rng);
                ry = d20(rng);
                map.get(rx, ry).unwrap().is_some()
            }
            {}

            map.check_put(rx, ry, village.borrow().deref().into());
            village.borrow_mut().limit -= 1;

            ants.push(Ant::new(rx, ry));
        }
    }

    // Move Ants, Generating Terrain, Until Map Full
    let mut progress = num_villages + 9;
    {
        println!("Moving Ants");
        while (!map.is_full()) {
            for ant in ants.iter_mut() {
                let source = Rc::clone(
                    tiles
                        .get(&map.get(ant.x, ant.y).unwrap().clone().unwrap().id)
                        .unwrap(),
                );

                ant.update(rng);
                let next_loc = source.borrow().roll(rng, &tiles, &corruption);
                if map.check_put(ant.x, ant.y, next_loc.borrow().deref().into()) {
                    next_loc.borrow_mut().limit -= 1;
                    progress += 1;
                }
            }
        }
    }

    // Generate Image
    {
        println!("Generating Image");
        let mut imagebuf: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(200, 200);
        for y in 0..20 {
            for x in 0..20 {
                for yi in 0..10 {
                    for xi in 0..10 {
                        let tile = map.get((x + 1) as usize, (y + 1) as usize)
                            .unwrap()
                            .clone()
                            .unwrap();
                        let mut color = tile.color;
                        if yi >= 3 && yi <= 6 && xi >= 3 && xi <= 6 {
                            color = tile.inner_color;
                        }
                        imagebuf.put_pixel(x * 10 + xi, y * 10 + yi, color)
                    }
                }
            }
        }
        imagebuf.save("image.png");
    }

}
